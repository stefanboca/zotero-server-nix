use std::{
    env,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{
    MySql, MySqlPool, Row, Transaction,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};
use topcoat::{
    Result as TopcoatResult,
    context::{Cx, app_context},
    cookie::RouterBuilderCookieExt,
    router::{
        IntoResponse, Response, Router,
        error::{bad_request, forbidden, see_other},
        query_params, route,
    },
    session::{self, RouterBuilderSessionExt, SessionConfig},
    view::{View, view},
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();
const FLOW_LIFETIME_SECONDS: u64 = 15 * 60;

type OidcClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
    oidc: OidcClient,
    http: reqwest::Client,
    config: Config,
    dataserver_super_password: String,
}

#[derive(Clone, Deserialize)]
struct Config {
    server: ServerConfig,
    database: DatabaseConfig,
    oidc: OidcConfig,
    identity: IdentityConfig,
    dataserver: DataserverConfig,
}

#[derive(Clone, Deserialize)]
struct ServerConfig {
    socket: PathBuf,
}

#[derive(Clone, Deserialize)]
struct DatabaseConfig {
    socket: PathBuf,
    user: String,
    name: String,
    max_connections: u32,
}

#[derive(Clone, Deserialize)]
struct OidcConfig {
    issuer: String,
    client_id: String,
    redirect_uri: String,
    scopes: Vec<String>,
}

#[derive(Clone, Deserialize)]
struct IdentityConfig {
    username_claim: String,
    real_name_claim: String,
    require_verified_email: bool,
}

#[derive(Clone, Deserialize)]
struct DataserverConfig {
    url: String,
}

struct LoginSessionInfo {
    status: String,
    user_id: Option<u64>,
    access: Option<Value>,
}

struct Flow {
    nonce: String,
    pkce_verifier: String,
    session_token: String,
    expires: u64,
}

#[topcoat::router::query_params(error = bad_request)]
struct LoginQuery {
    #[serde(rename = "session")]
    dataserver_session: String,
}

#[topcoat::router::query_params(error = bad_request)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (config_path, migrate_only) = arguments()?;
    let config: Config = toml::from_str(
        &std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading configuration {}", config_path.display()))?,
    )
    .context("decoding TOML configuration")?;

    let pool = database_pool(&config.database).await?;
    MIGRATOR
        .run(&pool)
        .await
        .context("running database migrations")?;
    if migrate_only {
        return Ok(());
    }

    let http = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("building HTTP client")?;
    let provider =
        CoreProviderMetadata::discover_async(IssuerUrl::new(config.oidc.issuer.clone())?, &http)
            .await
            .context("discovering OIDC provider")?;
    let oidc = CoreClient::from_provider_metadata(
        provider,
        ClientId::new(config.oidc.client_id.clone()),
        Some(ClientSecret::new(read_credential("oidc-client-secret")?)),
    )
    .set_redirect_uri(RedirectUrl::new(config.oidc.redirect_uri.clone())?);

    let dataserver_super_password = read_credential("dataserver-super-password")?;
    let socket = config.server.socket.clone();
    let state = AppState {
        pool,
        oidc,
        http,
        config,
        dataserver_super_password,
    };
    let router = Router::builder()
        .route(login)
        .route(callback)
        .route(health)
        .cookies()
        .sessions(
            SessionConfig::builder()
                .lifetime(Duration::from_secs(FLOW_LIFETIME_SECONDS))
                .build(),
        )
        .app_context(state)
        .build();

    match std::fs::remove_file(&socket) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("removing stale auth socket"),
    }
    let listener = tokio::net::UnixListener::bind(&socket).context("binding auth socket")?;
    topcoat::serve(listener, router)
        .await
        .context("serving auth portal")?;
    Ok(())
}

fn arguments() -> anyhow::Result<(PathBuf, bool)> {
    let mut arguments = env::args().skip(1);
    let mut config_path = None;
    let mut migrate_only = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| anyhow!("--config requires a path"))?,
                ));
            }
            "--migrate-only" => migrate_only = true,
            _ => bail!("unknown argument: {argument}"),
        }
    }
    Ok((
        config_path.ok_or_else(|| anyhow!("--config is required"))?,
        migrate_only,
    ))
}

async fn database_pool(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let options = MySqlConnectOptions::new()
        .username(&config.user)
        .database(&config.name)
        .socket(&config.socket);
    MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .connect_with(options)
        .await
        .context("connecting to MariaDB")
}

fn systemd_env(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!("{name} is required"))
}

fn read_credential(name: &str) -> anyhow::Result<String> {
    let directory = systemd_env("CREDENTIALS_DIRECTORY")?;
    let value = std::fs::read_to_string(PathBuf::from(directory).join(name))
        .with_context(|| format!("reading credential {name}"))?;
    Ok(value.trim().to_owned())
}

fn now() -> anyhow::Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn valid_session_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[route(GET "/login")]
async fn login(cx: &Cx) -> TopcoatResult<Response> {
    let query = query_params::<LoginQuery>(cx)?;
    if !valid_session_token(&query.dataserver_session) {
        return Err(bad_request("invalid Zotero login-session token").into());
    }

    let app = app_context::<AppState>(cx);
    let info = login_session_info(app, &query.dataserver_session).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    clear_presented_flow(&app.pool, cx).await?;

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut authorization = app.oidc.authorize_url(
        CoreAuthenticationFlow::AuthorizationCode,
        CsrfToken::new_random,
        Nonce::new_random,
    );
    for scope in &app.config.oidc.scopes {
        if scope != "openid" {
            authorization = authorization.add_scope(Scope::new(scope.clone()));
        }
    }
    let (url, csrf, nonce) = authorization.set_pkce_challenge(challenge).url();
    sqlx::query("DELETE FROM oidc_flows WHERE expires < ?")
        .bind(now()?)
        .execute(&app.pool)
        .await?;
    let browser_session = session::start(cx).await?;
    let expires = browser_session
        .expires_at
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let inserted = sqlx::query(
        "INSERT INTO oidc_flows (tokenHash, state, nonce, pkceVerifier, sessionToken, expires) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&browser_session.token_hash[..])
    .bind(csrf.secret())
    .bind(nonce.secret())
    .bind(verifier.secret())
    .bind(&query.dataserver_session)
    .bind(expires)
    .execute(&app.pool)
    .await;
    if let Err(error) = inserted {
        session::stop(cx).await?;
        if is_duplicate_key(&error) {
            return Err(bad_request(
                "an OIDC login is already being completed for this Zotero session",
            )
            .into());
        }
        return Err(error.into());
    }

    // A Topcoat redirect error bypasses the cookie layer's response finalizer.
    // Return a successful redirect so session::start can emit its cookie.
    see_other(url.as_str()).into_response(cx)
}

#[route(GET "/auth/callback")]
async fn callback(cx: &Cx) -> TopcoatResult<View> {
    let result = callback_inner(cx).await;
    if let Err(error) = &result {
        eprintln!("OIDC callback failed: {error:#}");
    }
    result
}

async fn callback_inner(cx: &Cx) -> TopcoatResult<View> {
    let query = query_params::<CallbackQuery>(cx)?;
    if let Some(error) = &query.error {
        let description = query.error_description.as_deref().unwrap_or(error);
        return Err(bad_request(format!("OIDC provider rejected login: {description}")).into());
    }
    let code = query
        .code
        .as_deref()
        .ok_or_else(|| bad_request("missing authorization code"))?;
    let state = query
        .state
        .as_deref()
        .ok_or_else(|| bad_request("missing OIDC state"))?;

    let app = app_context::<AppState>(cx);
    let token_hash = session::token_hash(cx)
        .await?
        .ok_or_else(|| bad_request("missing or invalid OIDC flow session"))?;
    let flow = consume_flow(&app.pool, &token_hash, state).await?;
    if flow.expires < now()? {
        return Err(bad_request("OIDC login flow expired").into());
    }

    let token_response = app
        .oidc
        .exchange_code(AuthorizationCode::new(code.to_owned()))?
        .set_pkce_verifier(PkceCodeVerifier::new(flow.pkce_verifier))
        .request_async(&app.http)
        .await
        .context("exchanging OIDC authorization code")?;
    let id_token = token_response
        .id_token()
        .ok_or_else(|| anyhow!("OIDC provider omitted ID token"))?;
    let verifier = app.oidc.id_token_verifier();
    let claims = id_token
        .claims(&verifier, &Nonce::new(flow.nonce))
        .context("validating OIDC ID token")?;

    if let Some(expected_hash) = claims.access_token_hash() {
        let actual_hash = AccessTokenHash::from_token(
            token_response.access_token(),
            id_token.signing_alg()?,
            id_token.signing_key(&verifier)?,
        )?;
        if &actual_hash != expected_hash {
            return Err(anyhow!("OIDC access-token hash did not match ID token").into());
        }
    }

    let issuer = claims.issuer().as_str().to_owned();
    let subject = claims.subject().as_str().to_owned();
    let mut all_claims = jwt_claims(id_token)?;
    if let Some(endpoint) = app.oidc.user_info_url() {
        let user_info: Value = app
            .http
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
        if value_string(&user_info, "sub") != Some(subject.as_str()) {
            return Err(anyhow!("OIDC UserInfo subject did not match ID token").into());
        }
        merge_claims(&mut all_claims, user_info)?;
    }

    let email = value_string(&all_claims, "email")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| bad_request("OIDC provider did not supply an email address"))?;
    let email_verified = value_at_path(&all_claims, "email_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if app.config.identity.require_verified_email && !email_verified {
        return Err(forbidden().into());
    }
    let username = value_string(&all_claims, &app.config.identity.username_claim);
    let real_name = value_string(&all_claims, &app.config.identity.real_name_claim);
    provision_and_complete_login_serialized(
        app,
        &flow.session_token,
        &issuer,
        &subject,
        email,
        username,
        real_name,
        email_verified,
    )
    .await?;
    sqlx::query("DELETE FROM oidc_flows WHERE tokenHash = ?")
        .bind(&token_hash[..])
        .execute(&app.pool)
        .await?;
    session::stop(cx).await?;

    success_page().await
}

#[route(GET "/auth/health")]
async fn health() -> TopcoatResult<&'static str> {
    Ok("ok")
}

async fn consume_flow(
    pool: &MySqlPool,
    token_hash: &session::TokenHash,
    state: &str,
) -> anyhow::Result<Flow> {
    let mut transaction = pool.begin().await?;
    let row = sqlx::query(
        "SELECT state, nonce, pkceVerifier, sessionToken, expires, consumed FROM oidc_flows WHERE tokenHash = ? FOR UPDATE",
    )
    .bind(&token_hash[..])
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| bad_request("unknown or already-used OIDC state"))?;
    if row.try_get::<u8, _>("consumed")? != 0 || row.try_get::<String, _>("state")? != state {
        return Err(bad_request("unknown or already-used OIDC state").into());
    }
    let flow = Flow {
        nonce: row.try_get("nonce")?,
        pkce_verifier: row.try_get("pkceVerifier")?,
        session_token: row.try_get("sessionToken")?,
        expires: row.try_get("expires")?,
    };
    sqlx::query("UPDATE oidc_flows SET consumed = 1 WHERE tokenHash = ?")
        .bind(&token_hash[..])
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(flow)
}

async fn clear_presented_flow(pool: &MySqlPool, cx: &Cx) -> TopcoatResult<()> {
    let Some(token_hash) = session::token_hash(cx).await? else {
        return Ok(());
    };
    let mut transaction = pool.begin().await?;
    let consumed = sqlx::query_scalar::<_, u8>(
        "SELECT consumed FROM oidc_flows WHERE tokenHash = ? FOR UPDATE",
    )
    .bind(&token_hash[..])
    .fetch_optional(&mut *transaction)
    .await?;
    if consumed.is_some_and(|consumed| consumed != 0) {
        return Err(bad_request("the current OIDC login is already being completed").into());
    }
    sqlx::query("DELETE FROM oidc_flows WHERE tokenHash = ?")
        .bind(&token_hash[..])
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn provision_user(
    pool: &MySqlPool,
    issuer: &str,
    subject: &str,
    email: &str,
    requested_username: Option<&str>,
    real_name: Option<&str>,
    email_verified: bool,
) -> TopcoatResult<u64> {
    sqlx::query(
        "INSERT INTO oidc_identities (issuer, subject, userID) VALUES (?, ?, NULL) ON DUPLICATE KEY UPDATE subject = VALUES(subject)",
    )
    .bind(issuer)
    .bind(subject)
    .execute(pool)
    .await?;

    let mut transaction = pool.begin().await?;
    let row = sqlx::query(
        "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_one(&mut *transaction)
    .await?;
    let existing = row.try_get::<Option<u64>, _>("userID")?;
    let user_id = if let Some(user_id) = existing {
        let role =
            sqlx::query_scalar::<_, String>("SELECT role FROM users WHERE userID = ? FOR UPDATE")
                .bind(user_id)
                .fetch_one(&mut *transaction)
                .await?;
        let banned =
            sqlx::query_scalar::<_, u8>("SELECT Banned FROM GDN_User WHERE UserID = ? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *transaction)
                .await?
                .unwrap_or(0);
        if role == "deleted" || banned != 0 {
            return Err(forbidden().into());
        }
        sqlx::query("UPDATE users SET email = ? WHERE userID = ?")
            .bind(email)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        user_id
    } else {
        let user_id = insert_user(
            &mut transaction,
            requested_username.unwrap_or_else(|| email.split('@').next().unwrap_or("user")),
            issuer,
            subject,
            email,
        )
        .await?;
        sqlx::query("UPDATE oidc_identities SET userID = ? WHERE issuer = ? AND subject = ?")
            .bind(user_id)
            .bind(issuer)
            .bind(subject)
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
        .bind(email)
        .bind(email_verified)
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
    if let Some(real_name) = real_name.filter(|value| !value.trim().is_empty()) {
        sqlx::query(
            "INSERT INTO users_meta (userID, metaKey, metaValue) VALUES (?, 'profile_realname', ?)",
        )
        .bind(user_id)
        .bind(real_name)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(user_id)
}

async fn insert_user(
    transaction: &mut Transaction<'_, MySql>,
    requested: &str,
    issuer: &str,
    subject: &str,
    email: &str,
) -> anyhow::Result<u64> {
    let base: String = requested
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(220)
        .collect();
    let base = if base.is_empty() { "user" } else { &base };
    let digest = Sha256::digest(format!("{issuer}\0{subject}"));
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
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
            Ok(result) => return Ok(result.last_insert_id()),
            Err(error) if is_duplicate_username(&error) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("could not allocate a unique username")
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

fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.code().as_deref() == Some("1062"))
}

fn jwt_claims<T: Serialize>(token: &T) -> anyhow::Result<Value> {
    let serialized = serde_json::to_value(token)?;
    let compact = serialized
        .as_str()
        .ok_or_else(|| anyhow!("ID token did not serialize as a JWT"))?;
    let payload = compact
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow!("ID token was not a compact JWT"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .context("decoding ID-token claims")?;
    serde_json::from_slice(&decoded).context("decoding ID-token claims JSON")
}

fn merge_claims(base: &mut Value, overlay: Value) -> anyhow::Result<()> {
    let base = base
        .as_object_mut()
        .ok_or_else(|| anyhow!("ID-token claims were not an object"))?;
    let overlay = overlay
        .as_object()
        .ok_or_else(|| anyhow!("UserInfo claims were not an object"))?;
    base.extend(overlay.clone());
    Ok(())
}

fn value_at_path<'a>(claims: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(claims, |value, part| value.get(part))
}

fn value_string<'a>(claims: &'a Value, path: &str) -> Option<&'a str> {
    value_at_path(claims, path).and_then(Value::as_str)
}

async fn login_session_info(app: &AppState, token: &str) -> anyhow::Result<LoginSessionInfo> {
    let response = app
        .http
        .get(format!(
            "{}/keys/sessions/{token}/info",
            app.config.dataserver.url.trim_end_matches('/')
        ))
        .basic_auth("zotero-auth", Some(&app.dataserver_super_password))
        .send()
        .await
        .context("requesting Zotero login-session information")?
        .error_for_status()
        .context("Dataserver rejected login-session information request")?;
    let value: Value = response
        .json()
        .await
        .context("decoding Zotero login-session information")?;
    Ok(LoginSessionInfo {
        status: value_string(&value, "status")
            .unwrap_or_default()
            .to_owned(),
        user_id: value.get("userID").and_then(Value::as_u64),
        access: value
            .get("access")
            .cloned()
            .filter(|value| !value.is_null()),
    })
}

async fn complete_login_session(
    app: &AppState,
    token: &str,
    user_id: u64,
    access: Option<Value>,
) -> anyhow::Result<()> {
    let access = access.unwrap_or_else(|| {
        json!({
            "user": {"library": true, "notes": true, "write": true},
            "groups": {"all": {"library": true, "write": true}}
        })
    });
    app.http
        .post(format!(
            "{}/keys/sessions/complete",
            app.config.dataserver.url.trim_end_matches('/')
        ))
        .basic_auth("zotero-auth", Some(&app.dataserver_super_password))
        .json(&json!({"sessionToken": token, "userID": user_id, "access": access}))
        .send()
        .await
        .context("completing Zotero login session")?
        .error_for_status()
        .context("Dataserver rejected login-session completion")?;
    Ok(())
}

async fn provision_and_complete_login_serialized(
    app: &AppState,
    token: &str,
    issuer: &str,
    subject: &str,
    email: &str,
    requested_username: Option<&str>,
    real_name: Option<&str>,
    email_verified: bool,
) -> TopcoatResult<()> {
    let user_id = provision_user(
        &app.pool,
        issuer,
        subject,
        email,
        requested_username,
        real_name,
        email_verified,
    )
    .await?;

    // Hold a transactional identity-row lock through Dataserver completion.
    // This protects Dataserver's non-atomic first-login shard initialization.
    // Unlike a named lock on a pooled connection, cancellation releases this
    // lock automatically when the transaction rolls back.
    let mut transaction = app.pool.begin().await?;
    let locked_user_id = sqlx::query_scalar::<_, Option<u64>>(
        "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_one(&mut *transaction)
    .await?;
    if locked_user_id != Some(user_id) {
        return Err(anyhow!("OIDC identity changed during login completion").into());
    }

    let info = login_session_info(app, token).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if info.user_id.is_some_and(|existing| existing != user_id) {
        return Err(forbidden().into());
    }
    complete_login_session(app, token, user_id, info.access).await?;
    transaction.commit().await?;
    Ok(())
}

async fn success_page() -> TopcoatResult<View> {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Zotero login complete"</title>
                <style>
                    "body{font:1rem system-ui,sans-serif;max-width:36rem;margin:15vh auto;padding:0 1.5rem;line-height:1.5;color:#222}main{border:1px solid #ddd;border-radius:.75rem;padding:2rem}h1{margin-top:0;color:#900}"
                </style>
            </head>
            <body>
                <main>
                    <h1>"Login complete"</h1>
                    <p>
                        "Your Zotero desktop session is ready. You can close this page and return to Zotero."
                    </p>
                </main>
            </body>
        </html>
    }
}
