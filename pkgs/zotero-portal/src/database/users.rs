use super::*;

impl UserSummary {
    pub fn display_name(&self) -> &str {
        self.real_name.as_deref().unwrap_or(&self.username)
    }
}

impl Database {
    pub async fn provision_user(&self, profile: &OidcProfile) -> anyhow::Result<Option<UserId>> {
        let mut transaction = self.db.begin().await?;
        sqlx::query!(
            "INSERT INTO oidc_identities (issuer, subject, userID) VALUES (?, ?, NULL) ON DUPLICATE KEY UPDATE subject = VALUES(subject)",
            profile.issuer,
            profile.subject
        )
        .execute(&mut *transaction)
        .await?;

        let row = sqlx::query!(
            "SELECT OI.userID AS `user_id?: UserId`, U.role AS `role?`, GU.Banned AS `banned?` \
             FROM oidc_identities OI \
             LEFT JOIN users U ON U.userID = OI.userID \
             LEFT JOIN GDN_User GU ON GU.UserID = OI.userID \
             WHERE OI.issuer = ? AND OI.subject = ? FOR UPDATE",
            profile.issuer,
            profile.subject
        )
        .fetch_one(&mut *transaction)
        .await?;
        let existing = row.user_id;
        let user_id = if let Some(user_id) = existing {
            let role = row
                .role
                .context("OIDC identity references a missing user")?;
            let banned = row.banned.unwrap_or(0);
            if role == "deleted" || banned != 0 {
                return Ok(None);
            }
            sqlx::query!(
                "UPDATE users SET email = ? WHERE userID = ?",
                profile.email,
                user_id
            )
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
            sqlx::query!(
                "UPDATE oidc_identities SET userID = ? WHERE issuer = ? AND subject = ?",
                user_id,
                profile.issuer,
                profile.subject
            )
            .execute(&mut *transaction)
            .await?;
            user_id
        };

        sqlx::query!("DELETE FROM users_email WHERE userID = ?", user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query!(
            "INSERT INTO users_email (userID, email, validated) VALUES (?, ?, ?)",
            user_id,
            profile.email,
            profile.email_verified
        )
        .execute(&mut *transaction)
        .await?;
        sqlx::query!(
            "INSERT INTO GDN_User (UserID, Banned) VALUES (?, 0) ON DUPLICATE KEY UPDATE UserID = VALUES(UserID)",
            user_id
        )
            .execute(&mut *transaction)
            .await?;
        if let Some(real_name) = profile
            .real_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            sqlx::query!(
                "INSERT IGNORE INTO users_meta (userID, metaKey, metaValue) \
                 VALUES (?, 'profile_realname', ?)",
                user_id,
                real_name
            )
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(Some(user_id))
    }

    pub async fn user_by_id(&self, user_id: UserId) -> anyhow::Result<Option<UserSummary>> {
        let row = sqlx::query_as!(
            UserSummary,
            "SELECT U.userID AS `user_id: UserId`, U.username, U.email, UM.metaValue AS `real_name?` \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             WHERE U.userID = ?",
            user_id
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    pub async fn user_profile(&self, user_id: UserId) -> anyhow::Result<UserProfile> {
        let row = sqlx::query!(
            "SELECT \
             MAX(CASE WHEN metaKey = 'profile_realname' THEN metaValue END) AS `real_name?`, \
             MAX(CASE WHEN metaKey = 'profile_location' THEN metaValue END) AS `location?`, \
             MAX(CASE WHEN metaKey = 'profile_disciplines' THEN metaValue END) AS `disciplines?`, \
             MAX(CASE WHEN metaKey = 'profile_affiliation' THEN metaValue END) AS `affiliation?`, \
             MAX(CASE WHEN metaKey = 'profile_about' THEN metaValue END) AS `about?` \
             FROM users_meta WHERE userID = ?",
            user_id
        )
        .fetch_one(&self.db)
        .await?;
        Ok(UserProfile {
            real_name: row.real_name,
            location: row.location,
            disciplines: row.disciplines,
            affiliation: row.affiliation,
            about: row.about,
        })
    }

    pub async fn update_user_profile(
        &self,
        user_id: UserId,
        profile: &UserProfile,
    ) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        for (key, value) in [
            ("profile_realname", profile.real_name.as_deref()),
            ("profile_location", profile.location.as_deref()),
            ("profile_disciplines", profile.disciplines.as_deref()),
            ("profile_affiliation", profile.affiliation.as_deref()),
            ("profile_about", profile.about.as_deref()),
        ] {
            if let Some(value) = value {
                sqlx::query(
                    "INSERT INTO users_meta (userID, metaKey, metaValue) VALUES (?, ?, ?) \
                     ON DUPLICATE KEY UPDATE metaValue = VALUES(metaValue)",
                )
                .bind(user_id)
                .bind(key)
                .bind(value)
                .execute(&mut *transaction)
                .await?;
            } else {
                sqlx::query("DELETE FROM users_meta WHERE userID = ? AND metaKey = ?")
                    .bind(user_id)
                    .bind(key)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn users_by_ids(&self, user_ids: &[UserId]) -> anyhow::Result<Vec<UserSummary>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let user_ids = serde_json::to_string(user_ids)?;
        let rows = sqlx::query_as!(
            UserSummary,
            "SELECT U.userID AS `user_id: UserId`, U.username, U.email, UM.metaValue AS `real_name?` \
             FROM JSON_TABLE(?, '$[*]' COLUMNS (userID INT UNSIGNED PATH '$')) IDs \
             JOIN users U ON U.userID = IDs.userID \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             ORDER BY U.username",
            user_ids
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows)
    }

    pub async fn user_by_identifier(
        &self,
        identifier: &str,
    ) -> anyhow::Result<Option<UserSummary>> {
        let row = sqlx::query_as!(
            UserSummary,
            "SELECT U.userID AS `user_id: UserId`, U.username, U.email, UM.metaValue AS `real_name?` \
             FROM users U \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             LEFT JOIN GDN_User GU ON GU.UserID = U.userID \
             WHERE (U.username = ? OR U.email = ?) \
             AND U.role <> 'deleted' AND COALESCE(GU.Banned, 0) = 0 \
             AND EXISTS (SELECT 1 FROM oidc_identities OI WHERE OI.userID = U.userID)",
            identifier,
            identifier
        )
        .fetch_optional(&self.db)
        .await?;
        Ok(row)
    }

    pub async fn search_users(
        &self,
        search: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<UserSummary>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as!(
            UserSummary,
            "SELECT U.userID AS `user_id: UserId`, U.username, U.email, UM.metaValue AS `real_name?` \
             FROM users U \
             CROSS JOIN (SELECT ? AS search) S \
             LEFT JOIN users_meta UM ON UM.userID = U.userID AND UM.metaKey = 'profile_realname' \
             LEFT JOIN GDN_User GU ON GU.UserID = U.userID \
             WHERE U.role <> 'deleted' AND COALESCE(GU.Banned, 0) = 0 \
             AND EXISTS (SELECT 1 FROM oidc_identities OI WHERE OI.userID = U.userID) \
             AND (LOCATE(S.search, U.username) > 0 \
                  OR LOCATE(S.search, U.email) > 0 \
                  OR LOCATE(S.search, COALESCE(UM.metaValue, '')) > 0) \
             ORDER BY CASE \
                 WHEN U.username = S.search OR U.email = S.search OR UM.metaValue = S.search THEN 0 \
                 WHEN U.username LIKE CONCAT(S.search, '%') \
                      OR U.email LIKE CONCAT(S.search, '%') \
                      OR UM.metaValue LIKE CONCAT(S.search, '%') THEN 1 \
                 ELSE 2 \
             END, COALESCE(NULLIF(UM.metaValue, ''), U.username), U.userID \
             LIMIT ?",
            search,
            limit as u64
        )
        .fetch_all(&self.db)
        .await?;
        Ok(rows)
    }
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
    for suffix in [String::new(), format!("-{suffix}")] {
        let prefix_length = 255usize.saturating_sub(suffix.len());
        let username = format!(
            "{}{}",
            base.chars().take(prefix_length).collect::<String>(),
            suffix
        );
        match sqlx::query!(
            "INSERT INTO users (username, password, email) VALUES (?, '!oidc-only!', ?)",
            username,
            email
        )
        .execute(&mut **transaction)
        .await
        {
            Ok(result) => {
                return UserId::try_from(result.last_insert_id())
                    .context("MariaDB returned a user ID outside its INT UNSIGNED range");
            }
            Err(error) if is_duplicate_username(&error) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("could not allocate the deterministic username")
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
