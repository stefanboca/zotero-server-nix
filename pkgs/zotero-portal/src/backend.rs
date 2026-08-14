use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, CsrfToken, Nonce, OAuth2TokenResponse as _,
    PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse as _, core::CoreAuthenticationFlow,
};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use sqlx::{MySql, MySqlPool, QueryBuilder, Row as _, Transaction};
use url::Url;

use crate::{
    config::{Config, Secrets},
    dataserver::{ConnectedClient, Dataserver, Group, GroupRole, LoginSessionInfo, StorageUsage},
    id::UserId,
    oidc::{Oidc, jwt_claims, merge_claims, value_at_path, value_string},
};

// TODO: each method accesses only one of db, dataserver, oidc, and client. split them
pub struct Backend {
    config: Arc<Config>,
    db: MySqlPool,
    dataserver: Dataserver,
    oidc: Oidc,
    client: reqwest::Client,
}

pub struct OidcAuthorization {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

#[derive(Clone)]
pub struct OidcFlow {
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub session_token: Option<String>,
    pub return_path: String,
    pub expires: u64,
}

#[derive(Clone)]
pub struct OidcProfile {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub username: Option<String>,
    pub real_name: Option<String>,
    pub email_verified: bool,
}

#[derive(Clone, Debug)]
pub struct UserSummary {
    pub user_id: UserId,
    pub username: String,
    pub email: String,
    pub real_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BrowserSession {
    pub handle: String,
    pub created_at: u64,
    pub last_seen_at: u64,
    pub expires_at: u64,
}

impl UserSummary {
    pub fn display_name(&self) -> &str {
        self.real_name.as_deref().unwrap_or(&self.username)
    }
}

pub struct WebLibraryAccess {
    pub api_key: String,
    pub api_url: Url,
    pub read_only: bool,
    pub streaming_url: Url,
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

pub struct IdentityLock<'a>(Transaction<'a, MySql>);

impl IdentityLock<'_> {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.0.commit().await.map_err(Into::into)
    }
}

impl Backend {
    pub async fn new(config: Arc<Config>, secrets: Secrets, db: MySqlPool) -> anyhow::Result<Self> {
        Ok(Self {
            oidc: Oidc::discover(&config, secrets.oidc_client_secret).await?,
            dataserver: Dataserver::new(&config, secrets.dataserver_super_password)?,
            config,
            db,
            client: reqwest::Client::new(),
        })
    }

    pub fn begin_oidc_authorization(&self) -> anyhow::Result<OidcAuthorization> {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut authorization = self.oidc.client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        for scope in &self.config.oidc.scopes {
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

    pub async fn exchange_oidc_code(
        &self,
        code: &str,
        flow: &OidcFlow,
    ) -> anyhow::Result<Option<OidcProfile>> {
        let token_response = self
            .oidc
            .client
            .exchange_code(AuthorizationCode::new(code.to_owned()))?
            .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier.clone()))
            .request_async(&self.oidc.http_client)
            .await
            .context("exchanging OIDC authorization code")?;
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("OIDC provider omitted ID token"))?;
        let verifier = self.oidc.client.id_token_verifier();
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
        if let Some(endpoint) = self.oidc.client.user_info_url() {
            let user_info: Value = self
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
        if self.config.oidc.require_verified_email && !email_verified {
            return Ok(None);
        }
        Ok(Some(OidcProfile {
            issuer: claims.issuer().as_str().to_owned(),
            subject: claims.subject().as_str().to_owned(),
            email: email.to_owned(),
            username: value_string(&all_claims, &self.config.oidc.username_claim)
                .map(str::to_owned),
            real_name: value_string(&all_claims, &self.config.oidc.real_name_claim)
                .map(str::to_owned),
            email_verified,
        }))
    }

    pub async fn delete_expired_oidc_flows(&self, now: u64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM oidc_flows WHERE expires < ?")
            .bind(now)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn create_oidc_flow(
        &self,
        token_hash: &[u8],
        flow: &OidcFlow,
    ) -> anyhow::Result<bool> {
        match
        sqlx::query("INSERT INTO oidc_flows (tokenHash, state, nonce, pkceVerifier, sessionToken, returnPath, expires) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(token_hash)
            .bind(&flow.state)
            .bind(&flow.nonce)
            .bind(&flow.pkce_verifier)
            .bind(&flow.session_token)
            .bind(&flow.return_path)
            .bind(flow.expires)
            .execute(&self.db)
            .await {
            Ok(_) => Ok(true),
            Err(error) if is_duplicate_key(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn consume_oidc_flow(
        &self,
        token_hash: &[u8],
        state: &str,
    ) -> anyhow::Result<Option<OidcFlow>> {
        let mut transaction = self.db.begin().await?;
        let row = sqlx::query(
                    "SELECT state, nonce, pkceVerifier, sessionToken, returnPath, expires, consumed FROM oidc_flows WHERE tokenHash = ? FOR UPDATE",
                )
                .bind(token_hash)
                .fetch_optional(&mut *transaction)
                .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        if row.try_get::<u8, _>("consumed")? != 0 || row.try_get::<String, _>("state")? != state {
            return Ok(None);
        }
        let flow = OidcFlow {
            state: row.try_get("state")?,
            nonce: row.try_get("nonce")?,
            pkce_verifier: row.try_get("pkceVerifier")?,
            session_token: row.try_get("sessionToken")?,
            return_path: row.try_get("returnPath")?,
            expires: row.try_get("expires")?,
        };
        sqlx::query("UPDATE oidc_flows SET consumed = 1 WHERE tokenHash = ?")
            .bind(token_hash)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(flow))
    }

    pub async fn clear_oidc_flow(&self, token_hash: &[u8]) -> anyhow::Result<bool> {
        let mut transaction = self.db.begin().await?;
        let consumed = sqlx::query_scalar::<_, u8>(
            "SELECT consumed FROM oidc_flows WHERE tokenHash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        if consumed.is_some_and(|consumed| consumed != 0) {
            return Ok(false);
        }
        sqlx::query("DELETE FROM oidc_flows WHERE tokenHash = ?")
            .bind(token_hash)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn delete_oidc_flow(&self, token_hash: &[u8]) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM oidc_flows WHERE tokenHash = ?")
            .bind(token_hash)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn provision_user(&self, profile: &OidcProfile) -> anyhow::Result<Option<UserId>> {
        sqlx::query(
            "INSERT INTO oidc_identities (issuer, subject, userID) VALUES (?, ?, NULL) ON DUPLICATE KEY UPDATE subject = VALUES(subject)",
        )
        .bind(&profile.issuer)
        .bind(&profile.subject)
        .execute(&self.db)
        .await?;

        let mut transaction = self.db.begin().await?;
        let row = sqlx::query(
            "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
        )
        .bind(&profile.issuer)
        .bind(&profile.subject)
        .fetch_one(&mut *transaction)
        .await?;
        let existing = row.try_get::<Option<UserId>, _>("userID")?;
        let user_id = if let Some(user_id) = existing {
            let role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM users WHERE userID = ? FOR UPDATE",
            )
            .bind(user_id)
            .fetch_one(&mut *transaction)
            .await?;
            let banned = sqlx::query_scalar::<_, u8>(
                "SELECT Banned FROM GDN_User WHERE UserID = ? FOR UPDATE",
            )
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?
            .unwrap_or(0);
            if role == "deleted" || banned != 0 {
                return Ok(None);
            }
            sqlx::query("UPDATE users SET email = ? WHERE userID = ?")
                .bind(&profile.email)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            user_id
        } else {
            let requested_username = profile
                .username
                .as_deref()
                .unwrap_or_else(|| profile.email.split('@').next().unwrap_or("user"));
            let user_id = insert_user(
                &mut transaction,
                requested_username,
                &profile.issuer,
                &profile.subject,
                &profile.email,
            )
            .await?;
            sqlx::query("UPDATE oidc_identities SET userID = ? WHERE issuer = ? AND subject = ?")
                .bind(user_id)
                .bind(&profile.issuer)
                .bind(&profile.subject)
                .execute(&mut *transaction)
                .await?;
            user_id
        };

        sqlx::query("DELETE FROM users_email WHERE userID = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO users_email (userID, email, validated) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(&profile.email)
            .bind(profile.email_verified)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO GDN_User (UserID, Banned) VALUES (?, 0) ON DUPLICATE KEY UPDATE UserID = VALUES(UserID)")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM users_meta WHERE userID = ? AND metaKey = 'profile_realname'")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        if let Some(real_name) = profile
            .real_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            sqlx::query(
                "INSERT INTO users_meta (userID, metaKey, metaValue) VALUES (?, 'profile_realname', ?)",
            )
            .bind(user_id)
            .bind(real_name)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(Some(user_id))
    }

    pub async fn lock_identity_for_login_completion(
        &self,
        profile: &OidcProfile,
        user_id: UserId,
    ) -> anyhow::Result<Option<IdentityLock<'_>>> {
        let mut transaction = self.db.begin().await?;
        let locked_user_id = sqlx::query_scalar::<_, Option<UserId>>(
            "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
        )
        .bind(&profile.issuer)
        .bind(&profile.subject)
        .fetch_one(&mut *transaction)
        .await?;

        if locked_user_id != Some(user_id) {
            return Ok(None);
        }
        Ok(Some(IdentityLock(transaction)))
    }

    pub async fn refresh_browser_session(
        &self,
        token_hash: &[u8],
        expires: u64,
    ) -> anyhow::Result<Option<UserId>> {
        let mut transaction = self.db.begin().await?;
        sqlx::query("DELETE FROM oidc_browser_sessions WHERE expiresAt <= CURRENT_TIMESTAMP(6)")
            .execute(&mut *transaction)
            .await?;
        let user_id = sqlx::query_scalar::<_, UserId>(
            "SELECT userID FROM oidc_browser_sessions WHERE tokenHash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        if user_id.is_some() {
            sqlx::query(
                "UPDATE oidc_browser_sessions SET expiresAt = FROM_UNIXTIME(?), lastSeenAt = CURRENT_TIMESTAMP(6) WHERE tokenHash = ?",
            )
            .bind(expires)
            .bind(token_hash)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(user_id)
    }

    pub async fn replace_browser_session(
        &self,
        old_token_hash: &[u8],
        new_token_hash: &[u8],
        user_id: UserId,
        expires: u64,
    ) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        sqlx::query(
            "DELETE FROM oidc_browser_sessions WHERE tokenHash = ? OR expiresAt <= CURRENT_TIMESTAMP(6)",
        )
        .bind(old_token_hash)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO oidc_browser_sessions (tokenHash, userID, expiresAt) VALUES (?, ?, FROM_UNIXTIME(?))",
        )
        .bind(new_token_hash)
        .bind(user_id)
        .bind(expires)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn delete_browser_session(&self, token_hash: &[u8]) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM oidc_browser_sessions WHERE tokenHash = ?")
            .bind(token_hash)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn browser_sessions_for_user(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Vec<BrowserSession>> {
        let mut transaction = self.db.begin().await?;
        sqlx::query("DELETE FROM oidc_browser_sessions WHERE expiresAt <= CURRENT_TIMESTAMP(6)")
            .execute(&mut *transaction)
            .await?;
        let rows = sqlx::query(
            "SELECT LOWER(HEX(tokenHash)) AS handle, \
             CAST(UNIX_TIMESTAMP(createdAt) AS UNSIGNED) AS createdAt, \
             CAST(UNIX_TIMESTAMP(lastSeenAt) AS UNSIGNED) AS lastSeenAt, \
             CAST(UNIX_TIMESTAMP(expiresAt) AS UNSIGNED) AS expiresAt \
             FROM oidc_browser_sessions WHERE userID = ? ORDER BY lastSeenAt DESC",
        )
        .bind(user_id)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        rows.into_iter()
            .map(|row| {
                Ok(BrowserSession {
                    handle: row.try_get("handle")?,
                    created_at: row.try_get("createdAt")?,
                    last_seen_at: row.try_get("lastSeenAt")?,
                    expires_at: row.try_get("expiresAt")?,
                })
            })
            .collect()
    }

    pub async fn revoke_browser_session(
        &self,
        user_id: UserId,
        handle: &str,
    ) -> anyhow::Result<bool> {
        Ok(sqlx::query(
            "DELETE FROM oidc_browser_sessions WHERE userID = ? AND tokenHash = UNHEX(?)",
        )
        .bind(user_id)
        .bind(handle)
        .execute(&self.db)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn revoke_all_browser_sessions(&self, user_id: UserId) -> anyhow::Result<u64> {
        Ok(
            sqlx::query("DELETE FROM oidc_browser_sessions WHERE userID = ?")
                .bind(user_id)
                .execute(&self.db)
                .await?
                .rows_affected(),
        )
    }

    pub async fn user_by_id(&self, user_id: UserId) -> anyhow::Result<Option<UserSummary>> {
        let row = sqlx::query(
            "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             WHERE U.userID = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(user_summary).transpose()?)
    }

    pub async fn users_by_ids(&self, user_ids: &[UserId]) -> anyhow::Result<Vec<UserSummary>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<MySql>::new(
            "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             WHERE U.userID IN (",
        );
        let mut ids = query.separated(", ");
        for user_id in user_ids {
            ids.push_bind(user_id);
        }
        ids.push_unseparated(") ORDER BY U.username");
        query
            .build()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(user_summary)
            .collect()
    }

    pub async fn user_by_identifier(
        &self,
        identifier: &str,
    ) -> anyhow::Result<Option<UserSummary>> {
        let row = sqlx::query(
            "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             LEFT JOIN GDN_User GU ON GU.UserID = U.userID \
             WHERE (U.username = ? OR U.email = ?) \
             AND U.role <> 'deleted' AND COALESCE(GU.Banned, 0) = 0 \
             AND EXISTS (SELECT 1 FROM oidc_identities OI WHERE OI.userID = U.userID)",
        )
        .bind(identifier)
        .bind(identifier)
        .fetch_optional(&self.db)
        .await?;
        Ok(row.map(user_summary).transpose()?)
    }

    pub async fn search_users(
        &self,
        query: &str,
        excluded_user_ids: &[UserId],
        limit: usize,
    ) -> anyhow::Result<Vec<UserSummary>> {
        let rows = sqlx::query(
            "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             LEFT JOIN GDN_User GU ON GU.UserID = U.userID \
             WHERE U.role <> 'deleted' AND COALESCE(GU.Banned, 0) = 0 \
             AND EXISTS (SELECT 1 FROM oidc_identities OI WHERE OI.userID = U.userID)",
        )
        .fetch_all(&self.db)
        .await?;

        let mut matches = rows
            .into_iter()
            .map(user_summary)
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .filter(|user| !excluded_user_ids.contains(&user.user_id))
            .filter_map(|user| user_search_score(query, &user).map(|score| (score, user)))
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

    pub async fn login_session_info(&self, token: &str) -> anyhow::Result<LoginSessionInfo> {
        self.dataserver.login_session_info(token).await
    }

    pub async fn complete_login_session(
        &self,
        token: &str,
        user_id: UserId,
        access: Option<Value>,
    ) -> anyhow::Result<()> {
        self.dataserver
            .complete_login_session(token, user_id, access)
            .await
    }

    pub async fn set_unlimited_storage(&self, user_id: UserId) -> anyhow::Result<()> {
        self.dataserver.set_unlimited_storage(user_id).await
    }

    pub async fn storage_usage(&self, user_id: UserId) -> anyhow::Result<StorageUsage> {
        self.dataserver.storage_usage(user_id).await
    }

    pub async fn connected_clients(&self, user_id: UserId) -> anyhow::Result<Vec<ConnectedClient>> {
        self.dataserver.connected_clients(user_id).await
    }

    pub async fn web_library_access(&self, user_id: UserId) -> anyhow::Result<WebLibraryAccess> {
        Ok(WebLibraryAccess {
            api_key: self.dataserver.web_library_api_key(user_id).await?,
            api_url: self.config.dataserver_url.clone(),
            read_only: false,
            streaming_url: self.config.stream_server_url.clone(),
        })
    }

    pub async fn translate(
        &self,
        endpoint: TranslationEndpoint,
        query: Option<&str>,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> anyhow::Result<TranslationResponse> {
        let mut url = self.config.translation_server_url.clone();
        url.set_path(endpoint.path());
        url.set_query(query);

        let mut request = self.client.post(url).body(body);
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
        self.dataserver
            .revoke_connected_client(user_id, handle)
            .await
    }

    pub async fn revoke_all_connected_clients(&self, user_id: UserId) -> anyhow::Result<usize> {
        self.dataserver.revoke_all_connected_clients(user_id).await
    }

    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        self.dataserver.groups_for_user(user_id).await
    }

    pub async fn group(&self, group_id: u64) -> anyhow::Result<Option<Group>> {
        self.dataserver.group(group_id).await
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        self.dataserver.create_private_group(user_id, name).await
    }

    pub async fn set_group_user(
        &self,
        group_id: u64,
        user_id: UserId,
        role: GroupRole,
    ) -> anyhow::Result<()> {
        self.dataserver
            .set_group_user(group_id, user_id, role)
            .await
    }

    pub async fn rename_group(&self, group: &Group, name: &str) -> anyhow::Result<()> {
        self.dataserver.rename_group(group, name).await
    }

    pub async fn remove_group_user(&self, group_id: u64, user_id: UserId) -> anyhow::Result<()> {
        self.dataserver.remove_group_user(group_id, user_id).await
    }

    pub async fn delete_group(&self, group_id: u64) -> anyhow::Result<()> {
        self.dataserver.delete_group(group_id).await
    }
}

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.code().as_deref() == Some("1062"))
}

async fn insert_user(
    transaction: &mut Transaction<'_, MySql>,
    requested: &str,
    issuer: &str,
    subject: &str,
    email: &str,
) -> anyhow::Result<UserId> {
    let base: String = requested
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(220)
        .collect();
    let base = if base.is_empty() { "user" } else { &base };
    let digest = Sha256::digest(format!("{issuer}\0{subject}"));
    let suffix = crate::hex_string(&digest[..6]);
    for attempt in 0..100 {
        let suffix = match attempt {
            0 => String::new(),
            1 => format!("-{suffix}"),
            _ => format!("-{suffix}-{attempt}"),
        };
        let prefix_length = 255usize.saturating_sub(suffix.len());
        let username = format!(
            "{}{}",
            base.chars().take(prefix_length).collect::<String>(),
            suffix
        );
        match sqlx::query(
            "INSERT INTO users (username, password, email) VALUES (?, '!oidc-only!', ?)",
        )
        .bind(username)
        .bind(email)
        .execute(&mut **transaction)
        .await
        {
            Ok(result) => return Ok(UserId::new(result.last_insert_id())),
            Err(error) if is_duplicate_username(&error) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("could not allocate a unique username")
}

fn user_summary(row: sqlx::mysql::MySqlRow) -> anyhow::Result<UserSummary> {
    Ok(UserSummary {
        user_id: row.try_get("userID")?,
        username: row.try_get("username")?,
        email: row.try_get("email")?,
        real_name: row.try_get("realName")?,
    })
}

fn is_duplicate_username(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(error) = error else {
        return false;
    };
    if error.code().as_deref() != Some("1062") {
        return false;
    }
    error
        .constraint()
        .is_some_and(|constraint| constraint == "username")
        || error.message().contains("for key 'username'")
        || error.message().contains("for key 'users.username'")
}

fn user_search_score(query: &str, user: &UserSummary) -> Option<usize> {
    let query = normalize_search(query);
    if query.chars().count() < 3 {
        return None;
    }

    let mut values = Vec::new();
    for value in [
        user.username.as_str(),
        user.email.as_str(),
        user.real_name.as_deref().unwrap_or_default(),
    ] {
        values.push(normalize_search(value));
        values.extend(
            value
                .split(|character: char| !character.is_alphanumeric())
                .map(normalize_search),
        );
    }
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            if value == query {
                Some(0)
            } else if value.starts_with(&query) {
                Some(10 + value.chars().count().saturating_sub(query.chars().count()))
            } else if let Some(position) = value.find(&query) {
                Some(30 + position)
            } else {
                let distance = edit_distance(&query, &value);
                let maximum_distance = usize::max(1, query.chars().count() / 3);
                (distance <= maximum_distance).then_some(50 + distance * 10)
            }
        })
        .min()
}

fn normalize_search(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.iter().enumerate() {
            current[right_index + 1] = usize::min(
                usize::min(current[right_index] + 1, previous[right_index + 1] + 1),
                previous[right_index] + usize::from(left_character != *right_character),
            );
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

pub fn session_handle(token_hash: &[u8]) -> String {
    crate::hex_string(token_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserSummary {
        UserSummary {
            user_id: UserId::new(1),
            username: "scauchy".to_owned(),
            email: "stefan.cauchy@example.com".to_owned(),
            real_name: Some("Stefan Cauchy".to_owned()),
        }
    }

    #[test]
    fn user_search_matches_names_usernames_emails_and_typos() {
        let user = user();
        assert!(user_search_score("Stefan", &user).is_some());
        assert!(user_search_score("scau", &user).is_some());
        assert!(user_search_score("example.com", &user).is_some());
        assert!(user_search_score("stefn", &user).is_some());
        assert!(user_search_score("caucy", &user).is_some());
    }

    #[test]
    fn user_search_rejects_short_and_unrelated_queries() {
        let user = user();
        assert_eq!(user_search_score("s", &user), None);
        assert_eq!(user_search_score("sc", &user), None);
        assert_eq!(user_search_score("completely unrelated", &user), None);
    }
}
