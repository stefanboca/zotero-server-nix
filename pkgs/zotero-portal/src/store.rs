use anyhow::bail;
use sha2::{Digest as _, Sha256};
use sqlx::{
    MySql, MySqlPool, QueryBuilder, Row, Transaction,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::{config::DatabaseConfig, id::UserId};

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
    session_token: Option<&str>,
    return_path: &str,
    expires: u64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO oidc_flows (tokenHash, state, nonce, pkceVerifier, sessionToken, returnPath, expires) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(state)
    .bind(nonce)
    .bind(pkce_verifier)
    .bind(session_token)
    .bind(return_path)
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

pub async fn delete_browser_session(pool: &MySqlPool, token_hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM oidc_browser_sessions WHERE tokenHash = ?")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn refresh_browser_session(
    pool: &MySqlPool,
    token_hash: &[u8],
    expires: u64,
) -> sqlx::Result<Option<UserId>> {
    let mut transaction = pool.begin().await?;
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
    pool: &MySqlPool,
    old_token_hash: &[u8],
    new_token_hash: &[u8],
    user_id: UserId,
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

pub async fn browser_sessions_for_user(
    pool: &MySqlPool,
    user_id: UserId,
) -> sqlx::Result<Vec<BrowserSession>> {
    let mut transaction = pool.begin().await?;
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
    pool: &MySqlPool,
    user_id: UserId,
    handle: &str,
) -> sqlx::Result<bool> {
    if handle.len() != 64 || !handle.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(false);
    }
    Ok(
        sqlx::query("DELETE FROM oidc_browser_sessions WHERE userID = ? AND tokenHash = UNHEX(?)")
            .bind(user_id)
            .bind(handle)
            .execute(pool)
            .await?
            .rows_affected()
            == 1,
    )
}

pub async fn revoke_all_browser_sessions(pool: &MySqlPool, user_id: UserId) -> sqlx::Result<u64> {
    Ok(
        sqlx::query("DELETE FROM oidc_browser_sessions WHERE userID = ?")
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

pub fn session_handle(token_hash: &[u8]) -> String {
    token_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub async fn user_by_id(pool: &MySqlPool, user_id: UserId) -> sqlx::Result<Option<UserSummary>> {
    let row = sqlx::query(
        "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
         FROM users U \
         LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
         WHERE U.userID = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(user_summary).transpose()
}

pub async fn users_by_ids(pool: &MySqlPool, user_ids: &[UserId]) -> sqlx::Result<Vec<UserSummary>> {
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
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(user_summary)
        .collect()
}

pub async fn oidc_user_by_identifier(
    pool: &MySqlPool,
    identifier: &str,
) -> sqlx::Result<Option<UserSummary>> {
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
    .fetch_optional(pool)
    .await?;
    row.map(user_summary).transpose()
}

pub async fn search_oidc_users(
    pool: &MySqlPool,
    query: &str,
    excluded_user_ids: &[UserId],
    limit: usize,
) -> sqlx::Result<Vec<UserSummary>> {
    let rows = sqlx::query(
        "SELECT U.userID, U.username, U.email, UM.metaValue AS realName \
         FROM users U \
         LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
         LEFT JOIN GDN_User GU ON GU.UserID = U.userID \
         WHERE U.role <> 'deleted' AND COALESCE(GU.Banned, 0) = 0 \
         AND EXISTS (SELECT 1 FROM oidc_identities OI WHERE OI.userID = U.userID)",
    )
    .fetch_all(pool)
    .await?;

    let mut matches = rows
        .into_iter()
        .map(user_summary)
        .collect::<sqlx::Result<Vec<_>>>()?
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

pub(crate) fn user_search_score(query: &str, user: &UserSummary) -> Option<usize> {
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

fn user_summary(row: sqlx::mysql::MySqlRow) -> sqlx::Result<UserSummary> {
    Ok(UserSummary {
        user_id: row.try_get("userID")?,
        username: row.try_get("username")?,
        email: row.try_get("email")?,
        real_name: row.try_get("realName")?,
    })
}

pub async fn provision_user(
    pool: &MySqlPool,
    profile: &OidcProfile,
) -> anyhow::Result<Option<UserId>> {
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
    let existing = row.try_get::<Option<UserId>, _>("userID")?;
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
    user_id: UserId,
) -> anyhow::Result<Option<Transaction<'a, MySql>>> {
    let mut transaction = pool.begin().await?;
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
    Ok(Some(transaction))
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
            Ok(result) => return Ok(UserId::new(result.last_insert_id())),
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
