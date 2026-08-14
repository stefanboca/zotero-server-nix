use super::*;
impl LoginCompletionLock<'_> {
    pub async fn commit(self) -> anyhow::Result<()> {
        self.0.commit().await.map_err(Into::into)
    }
}

impl Database {
    pub async fn lock_user_for_login_completion(
        &self,
        user_id: UserId,
    ) -> anyhow::Result<Option<LoginCompletionLock<'_>>> {
        let mut transaction = self.db.begin().await?;
        let locked_user_id =
            sqlx::query_scalar::<_, UserId>("SELECT userID FROM users WHERE userID = ? FOR UPDATE")
                .bind(user_id)
                .fetch_optional(&mut *transaction)
                .await?;

        if locked_user_id != Some(user_id) {
            return Ok(None);
        }
        Ok(Some(LoginCompletionLock(transaction)))
    }

    pub async fn refresh_browser_session(
        &self,
        dataserver: &Dataserver,
        token_hash: &[u8],
        expires_at: Timestamp,
    ) -> anyhow::Result<Option<UserId>> {
        self.delete_expired_browser_sessions(dataserver).await?;
        let mut transaction = self.db.begin().await?;
        let user_id = sqlx::query_scalar::<_, UserId>(
            "SELECT userID FROM browser_sessions WHERE tokenHash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        if user_id.is_some() {
            sqlx::query(
                "UPDATE browser_sessions SET expiresAt = FROM_UNIXTIME(?), lastSeenAt = CURRENT_TIMESTAMP(6) WHERE tokenHash = ?",
            )
            .bind(expires_at.as_second())
            .bind(token_hash)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(user_id)
    }

    pub async fn create_browser_session(
        &self,
        dataserver: &Dataserver,
        token_hash: &[u8],
        user_id: UserId,
        expires_at: Timestamp,
    ) -> anyhow::Result<()> {
        self.delete_expired_browser_sessions(dataserver).await?;
        let mut transaction = self.db.begin().await?;
        sqlx::query(
            "INSERT INTO browser_sessions (tokenHash, userID, expiresAt) VALUES (?, ?, FROM_UNIXTIME(?))",
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(expires_at.as_second())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn delete_browser_session(
        &self,
        dataserver: &Dataserver,
        token_hash: &[u8],
    ) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        let user_id = sqlx::query_scalar::<_, UserId>(
            "SELECT userID FROM browser_sessions WHERE tokenHash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(user_id) = user_id {
            dataserver
                .revoke_web_library_api_keys(user_id, &[session_handle(token_hash)])
                .await?;
        }
        sqlx::query("DELETE FROM browser_sessions WHERE tokenHash = ?")
            .bind(token_hash)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn delete_expired_browser_sessions(&self, dataserver: &Dataserver) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        let rows = sqlx::query(
            "SELECT userID, LOWER(HEX(tokenHash)) AS handle \
             FROM browser_sessions \
             WHERE expiresAt <= CURRENT_TIMESTAMP(6) FOR UPDATE",
        )
        .fetch_all(&mut *transaction)
        .await?;
        let mut handles_by_user = BTreeMap::<UserId, Vec<String>>::new();
        for row in rows {
            handles_by_user
                .entry(row.try_get("userID")?)
                .or_default()
                .push(row.try_get("handle")?);
        }
        for (user_id, handles) in handles_by_user {
            dataserver
                .revoke_web_library_api_keys(user_id, &handles)
                .await?;
        }
        sqlx::query("DELETE FROM browser_sessions WHERE expiresAt <= CURRENT_TIMESTAMP(6)")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn browser_sessions_for_user(
        &self,
        dataserver: &Dataserver,
        user_id: UserId,
    ) -> anyhow::Result<Vec<BrowserSession>> {
        self.delete_expired_browser_sessions(dataserver).await?;
        let rows = sqlx::query(
            "SELECT LOWER(HEX(tokenHash)) AS handle, \
             DATE_FORMAT(createdAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS createdAt, \
             DATE_FORMAT(lastSeenAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS lastSeenAt, \
             DATE_FORMAT(expiresAt, '%Y-%m-%dT%H:%i:%s.%fZ') AS expiresAt \
             FROM browser_sessions WHERE userID = ? ORDER BY lastSeenAt DESC",
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(BrowserSession {
                    handle: row.try_get("handle")?,
                    created_at: row.try_get::<String, _>("createdAt")?.parse()?,
                    last_seen_at: row.try_get::<String, _>("lastSeenAt")?.parse()?,
                    expires_at: row.try_get::<String, _>("expiresAt")?.parse()?,
                })
            })
            .collect()
    }

    pub async fn revoke_browser_session(
        &self,
        dataserver: &Dataserver,
        user_id: UserId,
        handle: &str,
    ) -> anyhow::Result<bool> {
        if handle.len() != 64 || !handle.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let mut transaction = self.db.begin().await?;
        let exists = sqlx::query_scalar::<_, UserId>(
            "SELECT userID FROM browser_sessions \
             WHERE userID = ? AND tokenHash = UNHEX(?) FOR UPDATE",
        )
        .bind(user_id)
        .bind(handle)
        .fetch_optional(&mut *transaction)
        .await?
        .is_some();
        if !exists {
            return Ok(false);
        }
        dataserver
            .revoke_web_library_api_keys(user_id, &[handle.to_owned()])
            .await?;
        sqlx::query("DELETE FROM browser_sessions WHERE userID = ? AND tokenHash = UNHEX(?)")
            .bind(user_id)
            .bind(handle)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn revoke_all_browser_sessions(
        &self,
        dataserver: &Dataserver,
        user_id: UserId,
    ) -> anyhow::Result<u64> {
        let mut transaction = self.db.begin().await?;
        let handles = sqlx::query_scalar::<_, String>(
            "SELECT LOWER(HEX(tokenHash)) FROM browser_sessions \
             WHERE userID = ? FOR UPDATE",
        )
        .bind(user_id)
        .fetch_all(&mut *transaction)
        .await?;
        dataserver
            .revoke_web_library_api_keys(user_id, &handles)
            .await?;
        let revoked = sqlx::query("DELETE FROM browser_sessions WHERE userID = ?")
            .bind(user_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        transaction.commit().await?;
        Ok(revoked)
    }

    pub async fn web_library_access(
        &self,
        dataserver: &Dataserver,
        user_id: UserId,
        token_hash: &[u8],
    ) -> anyhow::Result<WebLibraryAccess> {
        let mut transaction = self.db.begin().await?;
        let session_user_id = sqlx::query_scalar::<_, UserId>(
            "SELECT userID FROM browser_sessions WHERE tokenHash = ? FOR UPDATE",
        )
        .bind(token_hash)
        .fetch_optional(&mut *transaction)
        .await?;
        if session_user_id != Some(user_id) {
            anyhow::bail!("browser session disappeared while loading the Web Library");
        }
        let access = dataserver
            .web_library_access(user_id, &session_handle(token_hash))
            .await?;
        transaction.commit().await?;
        Ok(access)
    }
}
pub fn session_handle(token_hash: &[u8]) -> String {
    crate::hex_string(token_hash)
}
