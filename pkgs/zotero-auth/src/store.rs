use anyhow::bail;
use sha2::{Digest as _, Sha256};
use sqlx::{
    MySql, MySqlPool, Row, Transaction,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::config::DatabaseConfig;

pub struct OidcFlow {
    pub nonce: String,
    pub pkce_verifier: String,
    pub session_token: String,
    pub expires: u64,
}

pub struct OidcProfile {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub username: Option<String>,
    pub real_name: Option<String>,
    pub email_verified: bool,
}

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let options = MySqlConnectOptions::new()
        .username(&config.user)
        .database(&config.name)
        .socket(&config.socket);
    MySqlPoolOptions::new()
        .max_connections(config.max_connections)
        .connect_with(options)
        .await
        .map_err(Into::into)
}

pub async fn delete_expired_oidc_flows(pool: &MySqlPool, now: u64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM oidc_flows WHERE expires < ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_oidc_flow(
    pool: &MySqlPool,
    token_hash: &[u8],
    state: &str,
    nonce: &str,
    pkce_verifier: &str,
    session_token: &str,
    expires: u64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO oidc_flows (tokenHash, state, nonce, pkceVerifier, sessionToken, expires) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(state)
    .bind(nonce)
    .bind(pkce_verifier)
    .bind(session_token)
    .bind(expires)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_oidc_flow(
    pool: &MySqlPool,
    token_hash: &[u8],
    state: &str,
) -> sqlx::Result<Option<OidcFlow>> {
    let mut transaction = pool.begin().await?;
    let row = sqlx::query(
        "SELECT state, nonce, pkceVerifier, sessionToken, expires, consumed FROM oidc_flows WHERE tokenHash = ? FOR UPDATE",
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
        nonce: row.try_get("nonce")?,
        pkce_verifier: row.try_get("pkceVerifier")?,
        session_token: row.try_get("sessionToken")?,
        expires: row.try_get("expires")?,
    };
    sqlx::query("UPDATE oidc_flows SET consumed = 1 WHERE tokenHash = ?")
        .bind(token_hash)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(Some(flow))
}

pub async fn clear_oidc_flow(pool: &MySqlPool, token_hash: &[u8]) -> sqlx::Result<bool> {
    let mut transaction = pool.begin().await?;
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

pub async fn delete_oidc_flow(pool: &MySqlPool, token_hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM oidc_flows WHERE tokenHash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn refresh_browser_session(
    pool: &MySqlPool,
    token_hash: &[u8],
    expires: u64,
) -> sqlx::Result<Option<u64>> {
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM oidc_browser_sessions WHERE expiresAt <= CURRENT_TIMESTAMP(6)")
        .execute(&mut *transaction)
        .await?;
    let user_id = sqlx::query_scalar::<_, u64>(
        "SELECT userID FROM oidc_browser_sessions WHERE tokenHash = ? FOR UPDATE",
    )
    .bind(token_hash)
    .fetch_optional(&mut *transaction)
    .await?;
    if user_id.is_some() {
        sqlx::query(
            "UPDATE oidc_browser_sessions SET expiresAt = FROM_UNIXTIME(?) WHERE tokenHash = ?",
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
    pool: &MySqlPool,
    old_token_hash: &[u8],
    new_token_hash: &[u8],
    user_id: u64,
    expires: u64,
) -> sqlx::Result<()> {
    let mut transaction = pool.begin().await?;
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
    transaction.commit().await
}

pub async fn provision_user(
    pool: &MySqlPool,
    profile: &OidcProfile,
) -> anyhow::Result<Option<u64>> {
    sqlx::query(
        "INSERT INTO oidc_identities (issuer, subject, userID) VALUES (?, ?, NULL) ON DUPLICATE KEY UPDATE subject = VALUES(subject)",
    )
    .bind(&profile.issuer)
    .bind(&profile.subject)
    .execute(pool)
    .await?;

    let mut transaction = pool.begin().await?;
    let row = sqlx::query(
        "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
    )
    .bind(&profile.issuer)
    .bind(&profile.subject)
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

pub async fn lock_identity_for_login_completion<'a>(
    pool: &'a MySqlPool,
    profile: &OidcProfile,
    user_id: u64,
) -> anyhow::Result<Option<Transaction<'a, MySql>>> {
    let mut transaction = pool.begin().await?;
    let locked_user_id = sqlx::query_scalar::<_, Option<u64>>(
        "SELECT userID FROM oidc_identities WHERE issuer = ? AND subject = ? FOR UPDATE",
    )
    .bind(&profile.issuer)
    .bind(&profile.subject)
    .fetch_one(&mut *transaction)
    .await?;
    if locked_user_id != Some(user_id) {
        return Ok(None);
    }
    Ok(Some(transaction))
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

pub fn is_duplicate_key(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.code().as_deref() == Some("1062"))
}
