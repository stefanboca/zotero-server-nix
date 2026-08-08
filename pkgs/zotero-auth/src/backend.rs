use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
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
};

#[derive(Clone)]
pub struct Backend {
    inner: Arc<BackendInner>,
}

enum BackendInner {
    Production(ProductionBackend),
    Development(Mutex<DevelopmentState>),
}

struct ProductionBackend {
    config: Arc<Config>,
    database: MySqlPool,
    dataserver: Arc<Dataserver>,
    oidc: Arc<Oidc>,
}

struct DevelopmentState {
    users: BTreeMap<UserId, UserSummary>,
    identities: HashMap<(String, String), UserId>,
    groups: BTreeMap<u64, Group>,
    next_group_id: u64,
    storage: HashMap<UserId, StorageUsage>,
    connected_clients: HashMap<UserId, Vec<ConnectedClient>>,
    browser_sessions: HashMap<Vec<u8>, DevelopmentBrowserSession>,
    oidc_flows: HashMap<Vec<u8>, OidcFlow>,
    authorization_codes: HashMap<String, OidcProfile>,
    login_sessions: HashMap<String, LoginSessionInfo>,
    search_limits: HashMap<UserId, (u64, u32)>,
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
            })),
        }
    }

    pub fn development() -> Self {
        Self {
            inner: Arc::new(BackendInner::Development(Mutex::new(
                DevelopmentState::seeded(),
            ))),
        }
    }

    fn development_state(&self) -> anyhow::Result<MutexGuard<'_, DevelopmentState>> {
        let BackendInner::Development(state) = &*self.inner else {
            return Err(anyhow!("development backend operation used in production"));
        };
        state
            .lock()
            .map_err(|_| anyhow!("development backend state lock was poisoned"))
    }

    pub async fn begin_oidc_authorization(&self) -> anyhow::Result<OidcAuthorization> {
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
            BackendInner::Development(_) => {
                let state = CsrfToken::new_random();
                let nonce = Nonce::new_random();
                let code = CsrfToken::new_random();
                self.development_state()?.authorization_codes.insert(
                    code.secret().to_owned(),
                    DevelopmentState::researcher_profile(),
                );
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
            BackendInner::Development(_) => self
                .development_state()?
                .authorization_codes
                .remove(code)
                .map(Some)
                .ok_or_else(|| anyhow!("unknown or already-used mock authorization code")),
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
            BackendInner::Development(_) => {
                self.development_state()?
                    .oidc_flows
                    .retain(|_, flow| flow.expires >= now);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_oidc_flow(
        &self,
        token_hash: &[u8],
        state: &str,
        nonce: &str,
        pkce_verifier: &str,
        session_token: Option<&str>,
        return_path: &str,
        expires: u64,
    ) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                match store::create_oidc_flow(
                    &backend.database,
                    token_hash,
                    state,
                    nonce,
                    pkce_verifier,
                    session_token,
                    return_path,
                    expires,
                )
                .await
                {
                    Ok(()) => Ok(true),
                    Err(error) if store::is_duplicate_key(&error) => Ok(false),
                    Err(error) => Err(error.into()),
                }
            }
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
                if backend.oidc_flows.contains_key(token_hash) {
                    return Ok(false);
                }
                backend.oidc_flows.insert(
                    token_hash.to_vec(),
                    OidcFlow {
                        nonce: nonce.to_owned(),
                        pkce_verifier: pkce_verifier.to_owned(),
                        session_token: session_token.map(str::to_owned),
                        return_path: return_path.to_owned(),
                        expires,
                        state: state.to_owned(),
                    },
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                self.development_state()?.oidc_flows.remove(token_hash);
                Ok(true)
            }
        }
    }

    pub async fn delete_oidc_flow(&self, token_hash: &[u8]) -> anyhow::Result<()> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::delete_oidc_flow(&backend.database, token_hash).await?;
            }
            BackendInner::Development(_) => {
                self.development_state()?.oidc_flows.remove(token_hash);
            }
        }
        Ok(())
    }

    pub async fn provision_user(&self, profile: &OidcProfile) -> anyhow::Result<Option<UserId>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                store::provision_user(&backend.database, profile).await
            }
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
                let identity = (profile.issuer.clone(), profile.subject.clone());
                let user_id = backend
                    .identities
                    .get(&identity)
                    .copied()
                    .unwrap_or_else(|| {
                        let user_id = UserId::new(
                            backend
                                .users
                                .last_key_value()
                                .map_or(1, |(user_id, _)| user_id.get() + 1),
                        );
                        backend.identities.insert(identity, user_id);
                        user_id
                    });
                let username = profile.username.clone().unwrap_or_else(|| {
                    profile.email.split('@').next().unwrap_or("user").to_owned()
                });
                backend.users.insert(
                    user_id,
                    UserSummary {
                        user_id,
                        username,
                        email: profile.email.clone(),
                        real_name: profile.real_name.clone(),
                    },
                );
                Ok(Some(user_id))
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
            BackendInner::Development(_) => Ok((self
                .development_state()?
                .identities
                .get(&(profile.issuer.clone(), profile.subject.clone()))
                == Some(&user_id))
            .then_some(IdentityLock::Development)),
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
            BackendInner::Development(_) => {
                let now = epoch_seconds()?;
                let mut backend = self.development_state()?;
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
                .await?
            }
            BackendInner::Development(_) => {
                let now = epoch_seconds()?;
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                self.development_state()?
                    .browser_sessions
                    .remove(token_hash);
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
            BackendInner::Development(_) => {
                let mut sessions = self
                    .development_state()?
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                Ok(self.development_state()?.users.get(&user_id).cloned())
            }
        }
    }

    pub async fn users_by_ids(&self, user_ids: &[UserId]) -> anyhow::Result<Vec<UserSummary>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::users_by_ids(&backend.database, user_ids).await?)
            }
            BackendInner::Development(_) => {
                let backend = self.development_state()?;
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
            BackendInner::Development(_) => Ok(self
                .development_state()?
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
            BackendInner::Development(_) => {
                let backend = self.development_state()?;
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

    pub async fn allow_user_search(&self, user_id: UserId) -> anyhow::Result<bool> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                Ok(store::allow_user_search(&backend.database, user_id).await?)
            }
            BackendInner::Development(_) => {
                let now = epoch_seconds()?;
                let mut backend = self.development_state()?;
                let (window, count) = backend.search_limits.entry(user_id).or_insert((now, 0));
                if now.saturating_sub(*window) >= 60 {
                    *window = now;
                    *count = 0;
                }
                *count += 1;
                Ok(*count <= 30)
            }
        }
    }

    pub async fn login_session_info(&self, token: &str) -> anyhow::Result<LoginSessionInfo> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.login_session_info(token).await,
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                self.development_state()?.login_sessions.insert(
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
            BackendInner::Development(_) => {
                self.development_state()?
                    .storage
                    .entry(user_id)
                    .or_insert(StorageUsage {
                        total_mb: 0.0,
                        library_mb: 0.0,
                    });
                Ok(())
            }
        }
    }

    pub async fn storage_usage(&self, user_id: UserId) -> anyhow::Result<StorageUsage> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.storage_usage(user_id).await,
            BackendInner::Development(_) => Ok(self
                .development_state()?
                .storage
                .get(&user_id)
                .copied()
                .unwrap_or(StorageUsage {
                    total_mb: 0.0,
                    library_mb: 0.0,
                })),
        }
    }

    pub async fn connected_clients(&self, user_id: UserId) -> anyhow::Result<Vec<ConnectedClient>> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend.dataserver.connected_clients(user_id).await
            }
            BackendInner::Development(_) => Ok(self
                .development_state()?
                .connected_clients
                .get(&user_id)
                .cloned()
                .unwrap_or_default()),
        }
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
                let clients = backend.connected_clients.entry(user_id).or_default();
                let before = clients.len();
                clients.retain(|client| client.handle != handle);
                Ok(clients.len() != before)
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
            BackendInner::Development(_) => Ok(self
                .development_state()?
                .connected_clients
                .remove(&user_id)
                .map_or(0, |clients| clients.len())),
        }
    }

    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        match &*self.inner {
            BackendInner::Production(backend) => backend.dataserver.groups_for_user(user_id).await,
            BackendInner::Development(_) => Ok(self
                .development_state()?
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
            BackendInner::Development(_) => {
                Ok(self.development_state()?.groups.get(&group_id).cloned())
            }
        }
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        match &*self.inner {
            BackendInner::Production(backend) => {
                backend.dataserver.create_private_group(user_id, name).await
            }
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
                let group_id = backend.next_group_id;
                backend.next_group_id += 1;
                backend.groups.insert(
                    group_id,
                    Group {
                        id: group_id,
                        name: name.to_owned(),
                        owner: user_id,
                        admins: Vec::new(),
                        members: Vec::new(),
                        group_type: "Private".to_owned(),
                        library_editing: "members".to_owned(),
                        library_reading: "members".to_owned(),
                        file_editing: "members".to_owned(),
                        description: String::new(),
                        url: String::new(),
                        has_image: false,
                    },
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => {
                self.development_state()?
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
            BackendInner::Development(_) => {
                let mut backend = self.development_state()?;
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
            BackendInner::Development(_) => self
                .development_state()?
                .groups
                .remove(&group_id)
                .map(|_| ())
                .ok_or_else(|| anyhow!("group does not exist")),
        }
    }
}

impl DevelopmentState {
    fn seeded() -> Self {
        let now = epoch_seconds().unwrap_or(0);
        let users = [
            UserSummary {
                user_id: UserId::new(1),
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
        let profile = Self::researcher_profile();
        Self {
            users,
            identities: [(
                (profile.issuer.clone(), profile.subject.clone()),
                UserId::new(1),
            )]
            .into_iter()
            .collect(),
            groups: [
                (
                    1,
                    Group {
                        id: 1,
                        name: "Research group".to_owned(),
                        owner: UserId::new(1),
                        admins: vec![UserId::new(2)],
                        members: vec![UserId::new(3)],
                        group_type: "Private".to_owned(),
                        library_editing: "members".to_owned(),
                        library_reading: "members".to_owned(),
                        file_editing: "members".to_owned(),
                        description: String::new(),
                        url: String::new(),
                        has_image: false,
                    },
                ),
                (
                    2,
                    Group {
                        id: 2,
                        name: "Administered group".to_owned(),
                        owner: UserId::new(2),
                        admins: vec![UserId::new(1)],
                        members: vec![UserId::new(3)],
                        group_type: "Private".to_owned(),
                        library_editing: "members".to_owned(),
                        library_reading: "members".to_owned(),
                        file_editing: "members".to_owned(),
                        description: String::new(),
                        url: String::new(),
                        has_image: false,
                    },
                ),
                (
                    3,
                    Group {
                        id: 3,
                        name: "Member group".to_owned(),
                        owner: UserId::new(2),
                        admins: Vec::new(),
                        members: vec![UserId::new(1), UserId::new(3)],
                        group_type: "Private".to_owned(),
                        library_editing: "members".to_owned(),
                        library_reading: "members".to_owned(),
                        file_editing: "members".to_owned(),
                        description: String::new(),
                        url: String::new(),
                        has_image: false,
                    },
                ),
            ]
            .into_iter()
            .collect(),
            next_group_id: 4,
            storage: [(
                UserId::new(1),
                StorageUsage {
                    total_mb: 1280.0,
                    library_mb: 1024.0,
                },
            )]
            .into_iter()
            .collect(),
            connected_clients: [(
                UserId::new(1),
                vec![ConnectedClient {
                    handle: "22".repeat(32),
                    name: "Zotero Desktop".to_owned(),
                    date_added: "2026-08-01T10:00:00Z".to_owned(),
                    last_used: Some("2026-08-08T09:30:00Z".to_owned()),
                }],
            )]
            .into_iter()
            .collect(),
            browser_sessions: [(
                vec![0x11; 32],
                DevelopmentBrowserSession {
                    user_id: UserId::new(1),
                    created_at: now.saturating_sub(14 * 24 * 60 * 60),
                    last_seen_at: now.saturating_sub(2 * 60 * 60),
                    expires_at: now + 29 * 24 * 60 * 60,
                },
            )]
            .into_iter()
            .collect(),
            oidc_flows: HashMap::new(),
            authorization_codes: HashMap::new(),
            login_sessions: HashMap::new(),
            search_limits: HashMap::new(),
        }
    }

    fn researcher_profile() -> OidcProfile {
        OidcProfile {
            issuer: "https://mock-oidc.invalid".to_owned(),
            subject: "researcher".to_owned(),
            email: "researcher@example.com".to_owned(),
            username: Some("researcher".to_owned()),
            real_name: Some("Example Researcher".to_owned()),
            email_verified: true,
        }
    }
}

fn epoch_seconds() -> anyhow::Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn development_groups_cover_each_role() {
        let backend = Backend::development();
        let groups = backend
            .groups_for_user(UserId::new(1))
            .await
            .expect("development groups must load");
        let roles = groups
            .into_iter()
            .map(|group| {
                let role = group.role(UserId::new(1));
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
    async fn development_group_mutations_are_persistent() {
        let backend = Backend::development();
        let group_id = backend
            .create_private_group(UserId::new(1), "New group")
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
        assert_eq!(group.role(UserId::new(1)), Some(GroupRole::Admin));

        backend
            .remove_group_user(group_id, UserId::new(1))
            .await
            .expect("member removal must succeed");
        assert_eq!(
            backend
                .group(group_id)
                .await
                .expect("group lookup must succeed")
                .expect("new group must still exist")
                .role(UserId::new(1)),
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
        let user_id = UserId::new(1);
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
            .complete_login_session(token, UserId::new(1), None)
            .await
            .expect("login session completion must succeed");
        let completed = backend
            .login_session_info(token)
            .await
            .expect("completed login session lookup must succeed");
        assert_eq!(completed.status, "complete");
        assert_eq!(completed.user_id, Some(UserId::new(1)));
    }

    #[tokio::test]
    async fn development_oidc_codes_are_single_use() {
        let backend = Backend::development();
        let authorization = backend
            .begin_oidc_authorization()
            .await
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
