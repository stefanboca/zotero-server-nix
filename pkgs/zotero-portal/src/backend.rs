use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::{Context as _, anyhow};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, CsrfToken, Nonce, OAuth2TokenResponse as _,
    PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse as _, core::CoreAuthenticationFlow,
};
use serde_json::Value;
use sqlx::{MySql, MySqlPool, Transaction};

use crate::{
    config::Config,
    dataserver::{ConnectedClient, Dataserver, Group, GroupRole, LoginSessionInfo, StorageUsage},
    id::UserId,
    oidc::{Oidc, jwt_claims, merge_claims, value_at_path, value_string},
    store::{self, BrowserSession, OidcFlow, OidcProfile, UserSummary},
    epoch_seconds,
};

const DEVELOPMENT_USER_ID: UserId = UserId::new(1);
const DEVELOPMENT_API_KEY: &str = "development-api-key";
const DEVELOPMENT_OIDC_ISSUER: &str = "https://mock-oidc.invalid";
const DEVELOPMENT_OIDC_SUBJECT: &str = "researcher";

#[derive(Clone)]
pub struct Backend {
    inner: Arc<BackendInner>,
}

enum BackendInner {
    Production(ProductionBackend),
    Development(Box<Mutex<DevelopmentState>>),
}

struct ProductionBackend {
    config: Arc<Config>,
    database: MySqlPool,
    dataserver: Arc<Dataserver>,
    oidc: Arc<Oidc>,
    http_client: reqwest::Client,
}

struct DevelopmentState {
    users: BTreeMap<UserId, UserSummary>,
    groups: BTreeMap<u64, Group>,
    connected_clients: Vec<ConnectedClient>,
    browser_sessions: HashMap<Vec<u8>, DevelopmentBrowserSession>,
    oidc_flows: HashMap<Vec<u8>, OidcFlow>,
    authorization_codes: HashSet<String>,
    login_sessions: HashMap<String, LoginSessionInfo>,
}

struct DevelopmentBrowserSession {
    user_id: UserId,
    created_at: u64,
    last_seen_at: u64,
    expires_at: u64,
}

pub struct OidcAuthorization {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

pub struct WebLibraryAccess {
    pub api_key: String,
    pub api_url: String,
    pub read_only: bool,
    pub streaming_url: String,
}

pub struct WebLibraryApiResponse {
    pub status: u16,
    pub body: Value,
    pub raw_body: Option<Vec<u8>>,
    pub content_type: &'static str,
    pub total_results: Option<usize>,
    pub last_modified_version: Option<u64>,
}

#[derive(Clone, Copy)]
pub enum TranslationEndpoint {
    Export,
    Import,
    Search,
    Web,
}

impl TranslationEndpoint {
    fn path(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
            Self::Search => "search",
            Self::Web => "web",
        }
    }
}

pub struct TranslationResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub link: Option<String>,
}

pub enum IdentityLock<'a> {
    Production(Transaction<'a, MySql>),
    Development,
}

impl IdentityLock<'_> {
    pub async fn commit(self) -> anyhow::Result<()> {
        match self {
            Self::Production(transaction) => transaction.commit().await.map_err(Into::into),
            Self::Development => Ok(()),
        }
    }
}

impl Backend {
    pub fn production(
        config: Arc<Config>,
        database: MySqlPool,
        dataserver: Arc<Dataserver>,
        oidc: Arc<Oidc>,
    ) -> Self {
        Self {
            inner: Arc::new(BackendInner::Production(ProductionBackend {
                config,
                database,
                dataserver,
                oidc,
                http_client: reqwest::Client::new(),
            })),
        }
    }

    pub fn development() -> Self {
        Self {
            inner: Arc::new(BackendInner::Development(Box::new(Mutex::new(
                DevelopmentState::seeded(),
            )))),
        }
    }

    pub fn begin_oidc_authorization(&self) -> anyhow::Result<OidcAuthorization> {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        match &*self.inner {
            BackendInner::Production(backend) => {
                let mut authorization = backend.oidc.client.authorize_url(
                    CoreAuthenticationFlow::AuthorizationCode,
                    CsrfToken::new_random,
                    Nonce::new_random,
                );
                for scope in &backend.config.oidc.scopes {
                    if scope != "openid" {
                        authorization = authorization.add_scope(Scope::new(scope.clone()));
                    }
                }
                let (url, state, nonce) = authorization.set_pkce_challenge(challenge).url();
                Ok(OidcAuthorization {
                    url: url.to_string(),
                    state: state.secret().to_owned(),
                    nonce: nonce.secret().to_owned(),
                    pkce_verifier: verifier.secret().to_owned(),
                })
            }
            BackendInner::Development(development) => {
                let state = CsrfToken::new_random();
                let nonce = Nonce::new_random();
                let code = CsrfToken::new_random();
                development
                    .lock()
                    .unwrap()
                    .authorization_codes
                    .insert(code.secret().to_owned());
                let query = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("code", code.secret())
                    .append_pair("state", state.secret())
                    .finish();
                Ok(OidcAuthorization {
                    url: format!("/auth/oidc/callback?{query}"),
                    state: state.secret().to_owned(),
                    nonce: nonce.secret().to_owned(),
                    pkce_verifier: verifier.secret().to_owned(),
                })
            }
        }
    }

    pub async fn exchange_oidc_code(
        &self,
        code: &str,
        flow: &OidcFlow,
    ) -> anyhow::Result<Option<OidcProfile>> {
        match &*self.inner {
            BackendInner::Development(state) => {
                if state.lock().unwrap().authorization_codes.remove(code) {
                    Ok(Some(development_oidc_profile()))
                } else {
                    Err(anyhow!("unknown or already-used mock authorization code"))
                }
            }
            BackendInner::Production(backend) => {
                let token_response = backend
                    .oidc
                    .client
                    .exchange_code(AuthorizationCode::new(code.to_owned()))?
                    .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier.clone()))
                    .request_async(&backend.oidc.http_client)
                    .await
                    .context("exchanging OIDC authorization code")?;
                let id_token = token_response
                    .id_token()
                    .ok_or_else(|| anyhow!("OIDC provider omitted ID token"))?;
                let verifier = backend.oidc.client.id_token_verifier();
                let claims = id_token
                    .claims(&verifier, &Nonce::new(flow.nonce.clone()))
                    .context("validating OIDC ID token")?;

                if let Some(expected_hash) = claims.access_token_hash() {
                    let actual_hash = AccessTokenHash::from_token(
                        token_response.access_token(),
                        id_token.signing_alg()?,
                        id_token.signing_key(&verifier)?,
                    )?;
                    if &actual_hash != expected_hash {
                        return Err(anyhow!("OIDC access-token hash did not match ID token"));
                    }
                }

                let mut all_claims = jwt_claims(id_token)?;
                if let Some(endpoint) = backend.oidc.client.user_info_url() {
                    let user_info: Value = backend
                        .oidc
                        .http_client
                        .get(endpoint.as_str())
                        .bearer_auth(token_response.access_token().secret())
                        .send()
                        .await
                        .context("requesting OIDC UserInfo")?
                        .error_for_status()
                        .context("OIDC UserInfo returned an error")?
                        .json()
                        .await
                        .context("decoding OIDC UserInfo")?;
                    if value_string(&user_info, "sub") != Some(claims.subject().as_str()) {
                        return Err(anyhow!("OIDC UserInfo subject did not match ID token"));
                    }
                    merge_claims(&mut all_claims, user_info)?;
                }

                let email = value_string(&all_claims, "email")
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow!("OIDC provider did not supply an email address"))?;
                let email_verified = value_at_path(&all_claims, "email_verified")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if backend.config.identity.require_verified_email && !email_verified {
                    return Ok(None);
                }
                Ok(Some(OidcProfile {
                    issuer: claims.issuer().as_str().to_owned(),
                    subject: claims.subject().as_str().to_owned(),
                    email: email.to_owned(),
                    username: value_string(&all_claims, &backend.config.identity.username_claim)
                        .map(str::to_owned),
                    real_name: value_string(&all_claims, &backend.config.identity.real_name_claim)
                        .map(str::to_owned),
                    email_verified,
                }))
            }
        }
    }

    pub async fn delete_expired_oidc_flows(&self, now: u64) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::delete_expired_oidc_flows(&backend.database, now).await?;
            }
            BackendInner::Development(state) => {
                state
                    .lock()
                    .unwrap()
                    .oidc_flows
                    .retain(|_, flow| flow.expires >= now);
            }
        }
        Ok(())
    }

    pub async fn create_oidc_flow(
        &self,
        token_hash: &[u8],
        flow: &OidcFlow,
    ) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                match store::create_oidc_flow(&backend.database, token_hash, flow).await {
                    Ok(()) => Ok(true),
                    Err(error) if store::is_duplicate_key(&error) => Ok(false),
                    Err(error) => Err(error.into()),
                }
            }
            BackendInner::Development(development) => {
                let mut backend = development.lock().unwrap();
                if backend.oidc_flows.contains_key(token_hash) {
                    return Ok(false);
                }
                backend.oidc_flows.insert(
                    token_hash.to_vec(),
                    flow.clone(),
                );
                Ok(true)
            }
        }
    }

    pub async fn consume_oidc_flow(
        &self,
        token_hash: &[u8],
        state: &str,
    ) -> anyhow::Result<Option<OidcFlow>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::consume_oidc_flow(&backend.database, token_hash, state).await?)
            }
            BackendInner::Development(development) => {
                let mut backend = development.lock().unwrap();
                if backend
                    .oidc_flows
                    .get(token_hash)
                    .is_some_and(|flow| flow.state == state)
                {
                    Ok(backend.oidc_flows.remove(token_hash))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub async fn clear_oidc_flow(&self, token_hash: &[u8]) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::clear_oidc_flow(&backend.database, token_hash).await?)
            }
            BackendInner::Development(state) => {
                state.lock().unwrap().oidc_flows.remove(token_hash);
                Ok(true)
            }
        }
    }

    pub async fn delete_oidc_flow(&self, token_hash: &[u8]) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::delete_oidc_flow(&backend.database, token_hash).await?;
            }
            BackendInner::Development(state) => {
                state.lock().unwrap().oidc_flows.remove(token_hash);
            }
        }
        Ok(())
    }

    pub async fn provision_user(&self, profile: &OidcProfile) -> anyhow::Result<Option<UserId>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::provision_user(&backend.database, profile).await
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                if profile.issuer != DEVELOPMENT_OIDC_ISSUER
                    || profile.subject != DEVELOPMENT_OIDC_SUBJECT
                {
                    return Ok(None);
                }
                let username = profile.username.clone().unwrap_or_else(|| {
                    profile.email.split('@').next().unwrap_or("user").to_owned()
                });
                backend.users.insert(
                    DEVELOPMENT_USER_ID,
                    UserSummary {
                        user_id: DEVELOPMENT_USER_ID,
                        username,
                        email: profile.email.clone(),
                        real_name: profile.real_name.clone(),
                    },
                );
                Ok(Some(DEVELOPMENT_USER_ID))
            }
        }
    }

    pub async fn lock_identity_for_login_completion(
        &self,
        profile: &OidcProfile,
        user_id: UserId,
    ) -> anyhow::Result<Option<IdentityLock<'_>>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(
                    store::lock_identity_for_login_completion(&backend.database, profile, user_id)
                        .await?
                        .map(IdentityLock::Production),
                )
            }
            BackendInner::Development(_) => {
                Ok((user_id == DEVELOPMENT_USER_ID
                    && profile.issuer == DEVELOPMENT_OIDC_ISSUER
                    && profile.subject == DEVELOPMENT_OIDC_SUBJECT)
                    .then_some(IdentityLock::Development))
            }
        }
    }

    pub async fn refresh_browser_session(
        &self,
        token_hash: &[u8],
        expires: u64,
    ) -> anyhow::Result<Option<UserId>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::refresh_browser_session(&backend.database, token_hash, expires).await?)
            }
            BackendInner::Development(state) => {
                let now = epoch_seconds()?;
                let mut backend = state.lock().unwrap();
                backend
                    .browser_sessions
                    .retain(|_, browser_session| browser_session.expires_at > now);
                let Some(browser_session) = backend.browser_sessions.get_mut(token_hash) else {
                    return Ok(None);
                };
                browser_session.last_seen_at = now;
                browser_session.expires_at = expires;
                Ok(Some(browser_session.user_id))
            }
        }
    }

    pub async fn replace_browser_session(
        &self,
        old_token_hash: &[u8],
        new_token_hash: &[u8],
        user_id: UserId,
        expires: u64,
    ) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::replace_browser_session(
                    &backend.database,
                    old_token_hash,
                    new_token_hash,
                    user_id,
                    expires,
                )
                .await?;
            }
            BackendInner::Development(state) => {
                let now = epoch_seconds()?;
                let mut backend = state.lock().unwrap();
                backend.browser_sessions.remove(old_token_hash);
                backend.browser_sessions.insert(
                    new_token_hash.to_vec(),
                    DevelopmentBrowserSession {
                        user_id,
                        created_at: now,
                        last_seen_at: now,
                        expires_at: expires,
                    },
                );
            }
        }
        Ok(())
    }

    pub async fn delete_browser_session(&self, token_hash: &[u8]) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::delete_browser_session(&backend.database, token_hash).await?;
            }
            BackendInner::Development(state) => {
                state.lock().unwrap().browser_sessions.remove(token_hash);
            }
        }
        Ok(())
    }

    pub async fn browser_sessions_for_user(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<BrowserSession>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::browser_sessions_for_user(&backend.database, user_id).await?)
            }
            BackendInner::Development(state) => {
                let mut sessions = state
                    .lock()
                    .unwrap()
                    .browser_sessions
                    .iter()
                    .filter(|(_, browser_session)| browser_session.user_id == user_id)
                    .map(|(token_hash, browser_session)| BrowserSession {
                        handle: store::session_handle(token_hash),
                        created_at: browser_session.created_at,
                        last_seen_at: browser_session.last_seen_at,
                        expires_at: browser_session.expires_at,
                    })
                    .collect::<Vec<_>>();
                sessions.sort_by_key(|session| std::cmp::Reverse(session.last_seen_at));
                Ok(sessions)
            }
        }
    }

    pub async fn revoke_browser_session(
        &self,
        user_id: UserId,
        handle: &str,
    ) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::revoke_browser_session(&backend.database, user_id, handle).await?)
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let token_hash = backend
                    .browser_sessions
                    .iter()
                    .find(|(token_hash, session)| {
                        session.user_id == user_id && store::session_handle(token_hash) == handle
                    })
                    .map(|(token_hash, _)| token_hash.clone());
                Ok(token_hash
                    .and_then(|token_hash| backend.browser_sessions.remove(&token_hash))
                    .is_some())
            }
        }
    }

    pub async fn revoke_all_browser_sessions(&self, user_id: UserId) -> anyhow::Result<u64> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::revoke_all_browser_sessions(&backend.database, user_id).await?)
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let before = backend.browser_sessions.len();
                backend
                    .browser_sessions
                    .retain(|_, session| session.user_id != user_id);
                Ok((before - backend.browser_sessions.len()) as u64)
            }
        }
    }

    pub async fn user_by_id(&self, user_id: UserId) -> anyhow::Result<Option<UserSummary>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::user_by_id(&backend.database, user_id).await?)
            }
            BackendInner::Development(state) => {
                Ok(state.lock().unwrap().users.get(&user_id).cloned())
            }
        }
    }

    pub async fn users_by_ids(&self, user_ids: &[UserId]) -> anyhow::Result<Vec<UserSummary>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::users_by_ids(&backend.database, user_ids).await?)
            }
            BackendInner::Development(state) => {
                let backend = state.lock().unwrap();
                let mut users = user_ids
                    .iter()
                    .filter_map(|user_id| backend.users.get(user_id).cloned())
                    .collect::<Vec<_>>();
                users.sort_by(|left, right| left.username.cmp(&right.username));
                Ok(users)
            }
        }
    }

    pub async fn user_by_identifier(
        &self,
        identifier: &str,
    ) -> anyhow::Result<Option<UserSummary>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::oidc_user_by_identifier(&backend.database, identifier).await?)
            }
            BackendInner::Development(state) => Ok(state
                .lock()
                .unwrap()
                .users
                .values()
                .find(|user| {
                    user.username.eq_ignore_ascii_case(identifier)
                        || user.email.eq_ignore_ascii_case(identifier)
                })
                .cloned()),
        }
    }

    pub async fn search_users(
        &self,
        query: &str,
        excluded_user_ids: &[UserId],
        limit: usize,
    ) -> anyhow::Result<Vec<UserSummary>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(
                    store::search_oidc_users(&backend.database, query, excluded_user_ids, limit)
                        .await?,
                )
            }
            BackendInner::Development(state) => {
                let backend = state.lock().unwrap();
                let mut matches = backend
                    .users
                    .values()
                    .filter(|user| !excluded_user_ids.contains(&user.user_id))
                    .filter_map(|user| {
                        store::user_search_score(query, user).map(|score| (score, user.clone()))
                    })
                    .collect::<Vec<_>>();
                matches.sort_by(|(left_score, left), (right_score, right)| {
                    left_score
                        .cmp(right_score)
                        .then_with(|| left.display_name().cmp(right.display_name()))
                        .then_with(|| left.user_id.cmp(&right.user_id))
                });
                matches.truncate(limit);
                Ok(matches.into_iter().map(|(_, user)| user).collect())
            }
        }
    }

    pub async fn login_session_info(&self, token: &str) -> anyhow::Result<LoginSessionInfo> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.login_session_info(token).await,
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let info = backend
                    .login_sessions
                    .entry(token.to_owned())
                    .or_insert_with(|| LoginSessionInfo {
                        status: "pending".to_owned(),
                        user_id: None,
                        access: None,
                    });
                Ok(info.clone())
            }
        }
    }

    pub async fn complete_login_session(
        &self,
        token: &str,
        user_id: UserId,
        access: Option<Value>,
    ) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend
                    .dataserver
                    .complete_login_session(token, user_id, access)
                    .await
            }
            BackendInner::Development(state) => {
                state.lock().unwrap().login_sessions.insert(
                    token.to_owned(),
                    LoginSessionInfo {
                        status: "complete".to_owned(),
                        user_id: Some(user_id),
                        access,
                    },
                );
                Ok(())
            }
        }
    }

    pub async fn set_unlimited_storage(&self, user_id: UserId) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend.dataserver.set_unlimited_storage(user_id).await
            }
            BackendInner::Development(state) => {
                state
                    .lock()
                    .unwrap()
                    .users
                    .contains_key(&user_id)
                    .then_some(())
                    .ok_or_else(|| anyhow!("development user does not exist"))
            }
        }
    }

    pub async fn storage_usage(&self, user_id: UserId) -> anyhow::Result<StorageUsage> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.storage_usage(user_id).await,
            BackendInner::Development(_) => Ok(if user_id == DEVELOPMENT_USER_ID {
                StorageUsage {
                    total_mb: 1280.0,
                    library_mb: 1024.0,
                }
            } else {
                StorageUsage {
                    total_mb: 0.0,
                    library_mb: 0.0,
                }
            }),
        }
    }

    pub async fn connected_clients(&self, user_id: UserId) -> anyhow::Result<Vec<ConnectedClient>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend.dataserver.connected_clients(user_id).await
            }
            BackendInner::Development(state) => Ok(if user_id == DEVELOPMENT_USER_ID {
                state.lock().unwrap().connected_clients.clone()
            } else {
                Vec::new()
            }),
        }
    }

    pub async fn web_library_access(&self, user_id: UserId) -> anyhow::Result<WebLibraryAccess> {
        match &*self.inner {
            BackendInner::Production(backend) => Ok(WebLibraryAccess {
                api_key: backend.dataserver.web_library_api_key(user_id).await?,
                api_url: backend.config.dataserver.url.clone(),
                read_only: false,
                streaming_url: backend.config.streaming.url.clone(),
            }),
            BackendInner::Development(_) => {
                if user_id != DEVELOPMENT_USER_ID {
                    return Err(anyhow!("development user does not exist"));
                }
                Ok(WebLibraryAccess {
                    api_key: DEVELOPMENT_API_KEY.to_owned(),
                    api_url: "http://127.0.0.1:3000/api/".to_owned(),
                    read_only: true,
                    streaming_url: String::new(),
                })
            }
        }
    }

    pub fn web_library_api_response(
        &self,
        api_key: &str,
        path: &str,
        query: Option<&str>,
    ) -> Option<WebLibraryApiResponse> {
        let BackendInner::Development(state) = &*self.inner else {
            return None;
        };
        if api_key != DEVELOPMENT_API_KEY {
            return None;
        }
        let state = state.lock().unwrap();
        let user_id = DEVELOPMENT_USER_ID;

        let segments = path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        let requested_item_key = query.and_then(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .find_map(|(name, value)| (name == "itemKey").then(|| value.into_owned()))
        });
        match segments.as_slice() {
            ["users", id, "groups"] if parse_user_id(id) == Some(user_id) => {
                let groups = state
                    .groups
                    .values()
                    .filter(|group| group.role(user_id).is_some())
                    .map(mock_api_group)
                    .collect();
                Some(mock_api_list(groups))
            }
            ["users", id, path @ ..] if parse_user_id(id) == Some(user_id) => {
                let user = state.users.get(&user_id)?;
                mock_library_api_response(
                    api_key,
                    &MockLibrary {
                        library_type: "user",
                        id: user_id.get(),
                        name: &user.username,
                        parent_key: "DEVUSER1".to_owned(),
                        title: "A local Zotero library".to_owned(),
                    },
                    path,
                    requested_item_key.as_deref(),
                )
            }
            ["groups", id, path @ ..] => {
                let group = parse_group(&state, id, user_id)?;
                mock_library_api_response(
                    api_key,
                    &MockLibrary {
                        library_type: "group",
                        id: group.id,
                        name: &group.name,
                        parent_key: format!("DEVGRP{:02}", group.id),
                        title: format!("A paper shared with {}", group.name),
                    },
                    path,
                    requested_item_key.as_deref(),
                )
            }
            ["mock-files", key]
                if *key == mock_attachment_key(user_id.get())
                    || state.groups.values().any(|group| {
                        group.role(user_id).is_some() && *key == mock_attachment_key(group.id)
                    }) =>
            {
                Some(mock_api_raw("application/pdf", mock_pdf()))
            }
            _ => None,
        }
    }

    pub async fn translate(
        &self,
        endpoint: TranslationEndpoint,
        query: Option<&str>,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> anyhow::Result<TranslationResponse> {
        let BackendInner::Production(backend) = &*self.inner else {
            return Ok(TranslationResponse {
                status: 503,
                body: br#"{"message":"Translation is unavailable in development mode."}"#.to_vec(),
                content_type: Some("application/json; charset=utf-8".to_owned()),
                link: None,
            });
        };

        let mut url = url::Url::parse(&backend.config.translation_server.url)
            .context("parsing the translation server URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(anyhow!(
                "the translation server URL must be an HTTP URL without credentials or a query"
            ));
        }
        url.set_path(endpoint.path());
        url.set_query(query);

        let mut request = backend.http_client.post(url).body(body);
        if let Some(content_type) = content_type {
            request = request.header(reqwest::header::CONTENT_TYPE, content_type);
        }
        let response = request
            .send()
            .await
            .context("requesting Zotero translation")?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let link = response
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .await
            .context("reading the Zotero translation response")?
            .to_vec();
        Ok(TranslationResponse {
            status,
            body,
            content_type,
            link,
        })
    }

    pub async fn revoke_connected_client(
        &self,
        user_id: UserId,
        handle: &str,
    ) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend
                    .dataserver
                    .revoke_connected_client(user_id, handle)
                    .await
            }
            BackendInner::Development(state) => {
                if user_id != DEVELOPMENT_USER_ID {
                    return Ok(false);
                }
                let mut backend = state.lock().unwrap();
                let before = backend.connected_clients.len();
                backend
                    .connected_clients
                    .retain(|client| client.handle != handle);
                Ok(backend.connected_clients.len() != before)
            }
        }
    }

    pub async fn revoke_all_connected_clients(&self, user_id: UserId) -> anyhow::Result<usize> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend
                    .dataserver
                    .revoke_all_connected_clients(user_id)
                    .await
            }
            BackendInner::Development(state) => {
                if user_id != DEVELOPMENT_USER_ID {
                    return Ok(0);
                }
                let mut state = state.lock().unwrap();
                let revoked = state.connected_clients.len();
                state.connected_clients.clear();
                Ok(revoked)
            }
        }
    }

    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.groups_for_user(user_id).await,
            BackendInner::Development(state) => Ok(state
                .lock()
                .unwrap()
                .groups
                .values()
                .filter(|group| group.role(user_id).is_some())
                .cloned()
                .collect()),
        }
    }

    pub async fn group(&self, group_id: u64) -> anyhow::Result<Option<Group>> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.group(group_id).await,
            BackendInner::Development(state) => {
                Ok(state.lock().unwrap().groups.get(&group_id).cloned())
            }
        }
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend.dataserver.create_private_group(user_id, name).await
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let group_id = backend
                    .groups
                    .last_key_value()
                    .map_or(1, |(group_id, _)| *group_id + 1);
                backend.groups.insert(
                    group_id,
                    development_group(group_id, name, user_id, &[], &[]),
                );
                Ok(group_id)
            }
        }
    }

    pub async fn set_group_user(
        &self,
        group_id: u64,
        user_id: UserId,
        role: GroupRole,
    ) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend
                    .dataserver
                    .set_group_user(group_id, user_id, role)
                    .await
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let group = backend
                    .groups
                    .get_mut(&group_id)
                    .ok_or_else(|| anyhow!("group does not exist"))?;
                group.admins.retain(|candidate| *candidate != user_id);
                group.members.retain(|candidate| *candidate != user_id);
                match role {
                    GroupRole::Owner => {
                        let previous_owner = std::mem::replace(&mut group.owner, user_id);
                        if previous_owner != user_id {
                            group.admins.push(previous_owner);
                        }
                    }
                    GroupRole::Admin => group.admins.push(user_id),
                    GroupRole::Member => group.members.push(user_id),
                }
                Ok(())
            }
        }
    }

    pub async fn rename_group(&self, group: &Group, name: &str) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.rename_group(group, name).await,
            BackendInner::Development(state) => {
                state
                    .lock()
                    .unwrap()
                    .groups
                    .get_mut(&group.id)
                    .ok_or_else(|| anyhow!("group does not exist"))?
                    .name = name.to_owned();
                Ok(())
            }
        }
    }

    pub async fn remove_group_user(&self, group_id: u64, user_id: UserId) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend
                    .dataserver
                    .remove_group_user(group_id, user_id)
                    .await
            }
            BackendInner::Development(state) => {
                let mut backend = state.lock().unwrap();
                let group = backend
                    .groups
                    .get_mut(&group_id)
                    .ok_or_else(|| anyhow!("group does not exist"))?;
                group.admins.retain(|candidate| *candidate != user_id);
                group.members.retain(|candidate| *candidate != user_id);
                Ok(())
            }
        }
    }

    pub async fn delete_group(&self, group_id: u64) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.delete_group(group_id).await,
            BackendInner::Development(state) => state
                .lock()
                .unwrap()
                .groups
                .remove(&group_id)
                .map(|_| ())
                .ok_or_else(|| anyhow!("group does not exist")),
        }
    }
}

fn parse_user_id(value: &str) -> Option<UserId> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value != 0)
        .map(UserId::new)
}

fn parse_group<'a>(state: &'a DevelopmentState, value: &str, user_id: UserId) -> Option<&'a Group> {
    let group = state.groups.get(&value.parse().ok()?)?;
    group.role(user_id).is_some().then_some(group)
}

struct MockLibrary<'a> {
    library_type: &'static str,
    id: u64,
    name: &'a str,
    parent_key: String,
    title: String,
}

fn mock_library_api_response(
    api_key: &str,
    library: &MockLibrary<'_>,
    path: &[&str],
    requested_item_key: Option<&str>,
) -> Option<WebLibraryApiResponse> {
    let attachment_key = mock_attachment_key(library.id);
    let response = match path {
        ["settings", key] => mock_api_json(serde_json::json!({
            "key": key,
            "version": 1,
            "value": if matches!(*key, "tagColors" | "readerCustomThemes") {
                Value::Array(Vec::new())
            } else {
                Value::Null
            }
        })),
        ["collections"] | ["items", "top", "tags"] => mock_api_list(Vec::new()),
        ["items", "top"] => mock_api_list(vec![mock_api_item(
            library.library_type,
            library.id,
            library.name,
            &library.parent_key,
            &library.title,
        )]),
        ["items"] => {
            let values = match requested_item_key {
                Some(key) if key == attachment_key => vec![mock_api_attachment(
                    library.library_type,
                    library.id,
                    library.name,
                    &library.parent_key,
                )],
                Some(key) if key == library.parent_key => vec![mock_api_item(
                    library.library_type,
                    library.id,
                    library.name,
                    &library.parent_key,
                    &library.title,
                )],
                _ => Vec::new(),
            };
            mock_api_list(values)
        }
        ["items", key] if *key == attachment_key => mock_api_json(mock_api_attachment(
            library.library_type,
            library.id,
            library.name,
            &library.parent_key,
        )),
        ["items", key, "children"] if *key == attachment_key => {
            mock_api_list(Vec::new())
        }
        ["items", key, "file", "view", "url"] if *key == attachment_key => {
            mock_attachment_url(api_key, key)
        }
        _ => return None,
    };
    Some(response)
}

fn mock_api_json(body: Value) -> WebLibraryApiResponse {
    WebLibraryApiResponse {
        status: 200,
        body,
        raw_body: None,
        content_type: "application/json; charset=utf-8",
        total_results: None,
        last_modified_version: Some(1),
    }
}

fn mock_api_list(values: Vec<Value>) -> WebLibraryApiResponse {
    WebLibraryApiResponse {
        status: 200,
        total_results: Some(values.len()),
        body: Value::Array(values),
        raw_body: None,
        content_type: "application/json; charset=utf-8",
        last_modified_version: Some(1),
    }
}

fn mock_api_raw(content_type: &'static str, body: Vec<u8>) -> WebLibraryApiResponse {
    WebLibraryApiResponse {
        status: 200,
        body: Value::Null,
        raw_body: Some(body),
        content_type,
        total_results: None,
        last_modified_version: None,
    }
}

fn mock_attachment_key(library_id: u64) -> String {
    format!("PDF{:05}", library_id % 100_000)
}

fn mock_attachment_url(api_key: &str, attachment_key: &str) -> WebLibraryApiResponse {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("key", api_key)
        .finish();
    mock_api_raw(
        "text/plain; charset=utf-8",
        format!("/api/mock-files/{attachment_key}?{query}").into_bytes(),
    )
}

fn mock_pdf() -> Vec<u8> {
    let stream = b"BT\n/F1 24 Tf\n72 720 Td\n(Zotero Local Cloud development attachment) Tj\nET\n";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            String::from_utf8_lossy(stream)
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

fn mock_api_group(group: &Group) -> Value {
    serde_json::json!({
        "id": group.id,
        "version": 1,
        "links": {},
        "meta": {
            "created": "2026-08-01T00:00:00Z",
            "lastModified": "2026-08-08T00:00:00Z",
            "numItems": 1
        },
        "data": {
            "id": group.id,
            "version": 1,
            "name": group.name,
            "owner": group.owner,
            "type": group.group_type,
            "description": group.description,
            "url": group.url,
            "libraryEditing": group.library_editing,
            "libraryReading": group.library_reading,
            "fileEditing": group.file_editing,
            "admins": group.admins,
            "members": group.members
        }
    })
}

fn mock_api_item(
    library_type: &str,
    library_id: u64,
    library_name: &str,
    key: &str,
    title: &str,
) -> Value {
    let attachment_key = mock_attachment_key(library_id);
    let library_path = if library_type == "group" {
        "groups"
    } else {
        "users"
    };
    serde_json::json!({
        "key": key,
        "version": 1,
        "library": {
            "type": library_type,
            "id": library_id,
            "name": library_name,
            "links": {}
        },
        "links": {
            "attachment": {
                "href": format!("https://api.zotero.test/{library_path}/{library_id}/items/{attachment_key}"),
                "attachmentType": "application/pdf"
            }
        },
        "meta": {
            "creatorSummary": "Example Researcher",
            "parsedDate": "2026",
            "numChildren": 1
        },
        "data": {
            "key": key,
            "version": 1,
            "itemType": "journalArticle",
            "title": title,
            "creators": [{
                "creatorType": "author",
                "firstName": "Example",
                "lastName": "Researcher"
            }],
            "abstractNote": "",
            "publicationTitle": "Local Research",
            "volume": "1",
            "issue": "1",
            "pages": "1-8",
            "date": "2026",
            "series": "",
            "seriesTitle": "",
            "seriesText": "",
            "journalAbbreviation": "",
            "language": "en",
            "DOI": "",
            "ISSN": "",
            "shortTitle": "",
            "url": "",
            "accessDate": "",
            "archive": "",
            "archiveLocation": "",
            "libraryCatalog": "",
            "callNumber": "",
            "rights": "",
            "extra": "",
            "tags": [{ "tag": "development" }],
            "collections": [],
            "relations": {},
            "dateAdded": "2026-08-01T00:00:00Z",
            "dateModified": "2026-08-08T00:00:00Z"
        }
    })
}

fn mock_api_attachment(
    library_type: &str,
    library_id: u64,
    library_name: &str,
    parent_key: &str,
) -> Value {
    let key = mock_attachment_key(library_id);
    let library_path = if library_type == "group" {
        "groups"
    } else {
        "users"
    };
    serde_json::json!({
        "key": key,
        "version": 1,
        "library": {
            "type": library_type,
            "id": library_id,
            "name": library_name,
            "links": {}
        },
        "links": {
            "self": {
                "href": format!("https://api.zotero.test/{library_path}/{library_id}/items/{key}"),
                "type": "application/json"
            },
            "enclosure": {
                "href": format!("https://api.zotero.test/{library_path}/{library_id}/items/{key}/file"),
                "type": "application/pdf",
                "title": "development-paper.pdf",
                "length": 1024
            }
        },
        "meta": {
            "numChildren": 0
        },
        "data": {
            "key": key,
            "version": 1,
            "itemType": "attachment",
            "linkMode": "imported_file",
            "title": "Development paper PDF",
            "accessDate": "",
            "note": "",
            "contentType": "application/pdf",
            "charset": "",
            "filename": "development-paper.pdf",
            "md5": "",
            "mtime": 0,
            "parentItem": parent_key,
            "tags": [],
            "relations": {},
            "dateAdded": "2026-08-01T00:00:00Z",
            "dateModified": "2026-08-08T00:00:00Z"
        }
    })
}

fn development_group(
    id: u64,
    name: &str,
    owner: UserId,
    admins: &[UserId],
    members: &[UserId],
) -> Group {
    Group {
        id,
        name: name.to_owned(),
        owner,
        admins: admins.to_vec(),
        members: members.to_vec(),
        group_type: "Private".to_owned(),
        library_editing: "members".to_owned(),
        library_reading: "members".to_owned(),
        file_editing: "members".to_owned(),
        description: String::new(),
        url: String::new(),
        has_image: false,
    }
}

fn development_oidc_profile() -> OidcProfile {
    OidcProfile {
        issuer: DEVELOPMENT_OIDC_ISSUER.to_owned(),
        subject: DEVELOPMENT_OIDC_SUBJECT.to_owned(),
        email: "researcher@example.com".to_owned(),
        username: Some("researcher".to_owned()),
        real_name: Some("Example Researcher".to_owned()),
        email_verified: true,
    }
}

impl DevelopmentState {
    fn seeded() -> Self {
        let now = epoch_seconds().unwrap_or(0);
        let users = [
            UserSummary {
                user_id: DEVELOPMENT_USER_ID,
                username: "researcher".to_owned(),
                email: "researcher@example.com".to_owned(),
                real_name: Some("Example Researcher".to_owned()),
            },
            UserSummary {
                user_id: UserId::new(2),
                username: "administrator".to_owned(),
                email: "administrator@example.com".to_owned(),
                real_name: Some("Group Administrator".to_owned()),
            },
            UserSummary {
                user_id: UserId::new(3),
                username: "member".to_owned(),
                email: "member@example.com".to_owned(),
                real_name: Some("Group Member".to_owned()),
            },
            UserSummary {
                user_id: UserId::new(4),
                username: "new-member".to_owned(),
                email: "new-member@example.com".to_owned(),
                real_name: Some("New Member".to_owned()),
            },
        ]
        .into_iter()
        .map(|user| (user.user_id, user))
        .collect();
        Self {
            users,
            groups: [
                (
                    1,
                    development_group(
                        1,
                        "Research group",
                        DEVELOPMENT_USER_ID,
                        &[UserId::new(2)],
                        &[UserId::new(3)],
                    ),
                ),
                (
                    2,
                    development_group(
                        2,
                        "Administered group",
                        UserId::new(2),
                        &[DEVELOPMENT_USER_ID],
                        &[UserId::new(3)],
                    ),
                ),
                (
                    3,
                    development_group(
                        3,
                        "Member group",
                        UserId::new(2),
                        &[],
                        &[DEVELOPMENT_USER_ID, UserId::new(3)],
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            connected_clients: vec![ConnectedClient {
                handle: "22".repeat(32),
                name: "Zotero Desktop".to_owned(),
                date_added: "2026-08-01T10:00:00Z".to_owned(),
                last_used: Some("2026-08-08T09:30:00Z".to_owned()),
            }],
            browser_sessions: [(
                vec![0x11; 32],
                DevelopmentBrowserSession {
                    user_id: DEVELOPMENT_USER_ID,
                    created_at: now.saturating_sub(14 * 24 * 60 * 60),
                    last_seen_at: now.saturating_sub(2 * 60 * 60),
                    expires_at: now + 29 * 24 * 60 * 60,
                },
            )]
            .into_iter()
            .collect(),
            oidc_flows: HashMap::new(),
            authorization_codes: HashSet::new(),
            login_sessions: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn development_groups_cover_each_role() {
        let backend = Backend::development();
        let groups = backend
            .groups_for_user(DEVELOPMENT_USER_ID)
            .await
            .expect("development groups must load");
        let roles = groups
            .into_iter()
            .map(|group| {
                let role = group.role(DEVELOPMENT_USER_ID);
                (group.name, role)
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(roles.get("Research group"), Some(&Some(GroupRole::Owner)));
        assert_eq!(
            roles.get("Administered group"),
            Some(&Some(GroupRole::Admin))
        );
        assert_eq!(roles.get("Member group"), Some(&Some(GroupRole::Member)));
    }

    #[tokio::test]
    async fn development_web_library_api_uses_live_group_state() {
        let backend = Backend::development();
        let user_id = DEVELOPMENT_USER_ID;
        let access = backend
            .web_library_access(user_id)
            .await
            .expect("development Web Library access must load");

        let initial = backend
            .web_library_api_response(&access.api_key, "users/1/groups", None)
            .expect("development groups API must be enabled");
        assert!(initial.body.to_string().contains("Research group"));

        let group = backend
            .group(1)
            .await
            .expect("group lookup must succeed")
            .expect("seeded group must exist");
        backend
            .rename_group(&group, "Renamed through shared state")
            .await
            .expect("group rename must succeed");

        let updated = backend
            .web_library_api_response(&access.api_key, "users/1/groups", None)
            .expect("development groups API must remain enabled");
        assert!(
            updated
                .body
                .to_string()
                .contains("Renamed through shared state")
        );
        assert!(!updated.body.to_string().contains("Research group"));
        assert!(
            backend
                .web_library_api_response("invalid", "users/1/groups", None)
                .is_none()
        );
    }

    #[tokio::test]
    async fn development_web_library_api_filters_items_by_item_key() {
        let backend = Backend::development();
        let access = backend
            .web_library_access(DEVELOPMENT_USER_ID)
            .await
            .expect("development Web Library access must load");

        let response = backend
            .web_library_api_response(
                &access.api_key,
                "groups/2/items",
                Some("format=json&includeTrashed=0&itemKey=PDF00002"),
            )
            .expect("development items API must be enabled");

        assert_eq!(response.total_results, Some(1));
        assert_eq!(response.body[0]["key"], "PDF00002");
        assert_eq!(response.body[0]["data"]["itemType"], "attachment");
    }

    #[tokio::test]
    async fn development_group_mutations_are_persistent() {
        let backend = Backend::development();
        let group_id = backend
            .create_private_group(DEVELOPMENT_USER_ID, "New group")
            .await
            .expect("group creation must succeed");
        let group = backend
            .group(group_id)
            .await
            .expect("group lookup must succeed")
            .expect("new group must exist");
        assert_eq!(group.name, "New group");

        backend
            .rename_group(&group, "Renamed group")
            .await
            .expect("group rename must succeed");
        backend
            .set_group_user(group_id, UserId::new(4), GroupRole::Member)
            .await
            .expect("member addition must succeed");
        backend
            .set_group_user(group_id, UserId::new(4), GroupRole::Admin)
            .await
            .expect("member promotion must succeed");
        let group = backend
            .group(group_id)
            .await
            .expect("group lookup must succeed")
            .expect("new group must still exist");
        assert_eq!(group.name, "Renamed group");
        assert_eq!(group.role(UserId::new(4)), Some(GroupRole::Admin));

        backend
            .set_group_user(group_id, UserId::new(4), GroupRole::Owner)
            .await
            .expect("ownership transfer must succeed");
        let group = backend
            .group(group_id)
            .await
            .expect("group lookup must succeed")
            .expect("new group must still exist");
        assert_eq!(group.owner, UserId::new(4));
        assert_eq!(group.role(DEVELOPMENT_USER_ID), Some(GroupRole::Admin));

        backend
            .remove_group_user(group_id, DEVELOPMENT_USER_ID)
            .await
            .expect("member removal must succeed");
        assert_eq!(
            backend
                .group(group_id)
                .await
                .expect("group lookup must succeed")
                .expect("new group must still exist")
                .role(DEVELOPMENT_USER_ID),
            None
        );

        backend
            .delete_group(group_id)
            .await
            .expect("group deletion must succeed");
        assert!(
            backend
                .group(group_id)
                .await
                .expect("group lookup must succeed")
                .is_none()
        );
    }

    #[tokio::test]
    async fn development_session_and_client_mutations_are_persistent() {
        let backend = Backend::development();
        let user_id = DEVELOPMENT_USER_ID;
        let old_token = vec![0x33; 32];
        let new_token = vec![0x44; 32];
        backend
            .replace_browser_session(&old_token, &new_token, user_id, u64::MAX)
            .await
            .expect("browser session creation must succeed");
        assert_eq!(
            backend
                .refresh_browser_session(&new_token, u64::MAX)
                .await
                .expect("browser session refresh must succeed"),
            Some(user_id)
        );
        let handle = store::session_handle(&new_token);
        assert!(
            backend
                .revoke_browser_session(user_id, &handle)
                .await
                .expect("browser session revocation must succeed")
        );
        assert_eq!(
            backend
                .refresh_browser_session(&new_token, u64::MAX)
                .await
                .expect("revoked session lookup must succeed"),
            None
        );

        let client = backend
            .connected_clients(user_id)
            .await
            .expect("connected client lookup must succeed")
            .into_iter()
            .next()
            .expect("seeded client must exist");
        assert!(
            backend
                .revoke_connected_client(user_id, &client.handle)
                .await
                .expect("connected client revocation must succeed")
        );
        assert!(
            backend
                .connected_clients(user_id)
                .await
                .expect("connected client lookup must succeed")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn development_desktop_login_sessions_are_persistent() {
        let backend = Backend::development();
        let token = "0123456789abcdefghijklmnopqrstuv";
        let pending = backend
            .login_session_info(token)
            .await
            .expect("login session lookup must succeed");
        assert_eq!(pending.status, "pending");

        backend
            .complete_login_session(token, DEVELOPMENT_USER_ID, None)
            .await
            .expect("login session completion must succeed");
        let completed = backend
            .login_session_info(token)
            .await
            .expect("completed login session lookup must succeed");
        assert_eq!(completed.status, "complete");
        assert_eq!(completed.user_id, Some(DEVELOPMENT_USER_ID));
    }

    #[tokio::test]
    async fn development_oidc_codes_are_single_use() {
        let backend = Backend::development();
        let authorization = backend
            .begin_oidc_authorization()
            .expect("authorization must start");
        let url = url::Url::parse(&format!("http://localhost{}", authorization.url))
            .expect("mock callback URL must be valid");
        let code = url
            .query_pairs()
            .find(|(name, _)| name == "code")
            .map(|(_, value)| value.into_owned())
            .expect("mock callback must contain a code");
        let flow = OidcFlow {
            nonce: authorization.nonce,
            pkce_verifier: authorization.pkce_verifier,
            session_token: None,
            return_path: "/".to_owned(),
            expires: u64::MAX,
            state: authorization.state,
        };

        assert!(backend.exchange_oidc_code(&code, &flow).await.is_ok());
        assert!(backend.exchange_oidc_code(&code, &flow).await.is_err());
    }
}
