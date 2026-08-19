use super::*;

impl Dataserver {
    pub async fn web_library_access(
        &self,
        user_id: UserId,
        session_handle: &str,
    ) -> anyhow::Result<WebLibraryAccess> {
        Ok(WebLibraryAccess {
            api_key: self.web_library_api_key(user_id, session_handle).await?,
            api_url: self.config.dataserver_url.clone(),
            streaming_url: self.config.stream_server_url.clone(),
        })
    }

    async fn api_keys(&self, user_id: UserId) -> anyhow::Result<Vec<ApiKey>> {
        let response = self
            .super_request(reqwest::Method::GET, &format!("/users/{user_id}/keys"))
            .send()
            .await
            .context("listing connected Zotero clients")?
            .error_for_status()
            .context("Dataserver rejected connected-client listing")?;
        let body = response
            .text()
            .await
            .context("reading connected-client listing")?;
        if body.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&body).context("decoding connected-client listing")
    }

    pub async fn web_library_api_key(
        &self,
        user_id: UserId,
        session_handle: &str,
    ) -> anyhow::Result<String> {
        if let Some(key) = self
            .web_library_key_by_name(user_id, session_handle)
            .await?
        {
            return Ok(key.key);
        }

        let response = self
            .super_request(reqwest::Method::POST, &format!("/users/{user_id}/keys"))
            .json(&json!({
                "name": session_handle,
                "webUI": true,
                "access": {
                    "user": {"library": true, "notes": true, "files": true, "write": true},
                    "groups": {"all": {"library": true, "write": true}}
                }
            }))
            .send()
            .await
            .context("creating the Zotero Web Library API key")?
            .error_for_status()
            .context("Dataserver rejected the Zotero Web Library API key")?;
        let key: ApiKey = response
            .json()
            .await
            .context("decoding the Zotero Web Library API key")?;
        if key.user_id != user_id
            || !key.web_ui
            || key.name != session_handle
            || !key.has_full_web_library_access()
        {
            anyhow::bail!("Dataserver created an invalid Zotero Web Library API key");
        }
        Ok(key.key)
    }

    async fn web_library_key_by_name(
        &self,
        user_id: UserId,
        session_handle: &str,
    ) -> anyhow::Result<Option<ApiKey>> {
        if session_handle.len() != 64
            || !session_handle
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            anyhow::bail!("invalid Web Library session handle");
        }
        let response = self
            .super_request(
                reqwest::Method::GET,
                &format!("/users/{user_id}/keys/web-ui/{session_handle}"),
            )
            .send()
            .await
            .context("looking up the Zotero Web Library API key")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let key = response
            .error_for_status()
            .context("Dataserver rejected the Zotero Web Library API key lookup")?
            .json::<ApiKey>()
            .await
            .context("decoding the Zotero Web Library API key")?;
        if key.user_id != user_id
            || !key.web_ui
            || key.name != session_handle
            || !key.has_full_web_library_access()
        {
            anyhow::bail!("Dataserver returned an invalid Zotero Web Library API key");
        }
        Ok(Some(key))
    }

    async fn delete_api_key(&self, key: &str) -> anyhow::Result<()> {
        self.super_request(reqwest::Method::DELETE, &format!("/keys/{key}"))
            .send()
            .await
            .context("removing a Zotero API key")?
            .error_for_status()
            .context("Dataserver rejected Zotero API key removal")?;
        Ok(())
    }

    pub async fn access_keys(&self, user_id: UserId) -> anyhow::Result<Vec<ApiKey>> {
        let mut keys = self
            .api_keys(user_id)
            .await?
            .into_iter()
            .filter(|key| key.user_id == user_id && !key.web_ui)
            .collect::<Vec<_>>();
        keys.sort_by_cached_key(|key| (key.name.to_lowercase(), key.name.clone(), key.key.clone()));
        Ok(keys)
    }

    pub async fn access_key(
        &self,
        user_id: UserId,
        key_value: &str,
    ) -> anyhow::Result<Option<ApiKey>> {
        Ok(self
            .access_keys(user_id)
            .await?
            .into_iter()
            .find(|key| key.key == key_value))
    }

    pub async fn create_access_key(
        &self,
        user_id: UserId,
        name: &str,
        access: &ApiKeyAccess,
    ) -> anyhow::Result<ApiKey> {
        #[derive(Serialize)]
        struct CreateKey<'a> {
            name: &'a str,
            access: &'a ApiKeyAccess,
        }

        let key = self
            .super_request(reqwest::Method::POST, &format!("/users/{user_id}/keys"))
            .json(&CreateKey { name, access })
            .send()
            .await
            .context("creating a Zotero API key")?
            .error_for_status()
            .context("Dataserver rejected Zotero API key creation")?
            .json::<ApiKey>()
            .await
            .context("decoding created Zotero API key")?;
        if key.user_id != user_id || key.web_ui {
            anyhow::bail!("Dataserver created an API key for the wrong user");
        }
        Ok(key)
    }

    pub async fn update_access_key(
        &self,
        user_id: UserId,
        key_value: &str,
        name: &str,
        access: &ApiKeyAccess,
    ) -> anyhow::Result<Option<ApiKey>> {
        if self.access_key(user_id, key_value).await?.is_none() {
            return Ok(None);
        }

        #[derive(Serialize)]
        struct UpdateKey<'a> {
            key: &'a str,
            name: &'a str,
            access: &'a ApiKeyAccess,
        }

        let key = self
            .super_request(reqwest::Method::PUT, &format!("/keys/{key_value}"))
            .json(&UpdateKey {
                key: key_value,
                name,
                access,
            })
            .send()
            .await
            .context("updating a Zotero API key")?
            .error_for_status()
            .context("Dataserver rejected Zotero API key update")?
            .json::<ApiKey>()
            .await
            .context("decoding updated Zotero API key")?;
        if key.user_id != user_id || key.web_ui || key.key != key_value {
            anyhow::bail!("Dataserver updated an API key for the wrong user");
        }
        Ok(Some(key))
    }

    pub async fn delete_access_key(
        &self,
        user_id: UserId,
        key_value: &str,
    ) -> anyhow::Result<bool> {
        if self.access_key(user_id, key_value).await?.is_none() {
            return Ok(false);
        }
        self.delete_api_key(key_value).await?;
        Ok(true)
    }

    pub async fn revoke_web_library_api_keys(
        &self,
        user_id: UserId,
        session_handles: &[String],
    ) -> anyhow::Result<()> {
        for session_handle in session_handles {
            if let Some(key) = self
                .web_library_key_by_name(user_id, session_handle)
                .await?
            {
                self.delete_api_key(&key.key).await?;
            }
        }
        Ok(())
    }
}

impl ApiKey {
    fn has_full_web_library_access(&self) -> bool {
        let Some(all_groups) = self.access.groups.get(&ApiKeyGroupTarget::All) else {
            return false;
        };
        self.access.user.library
            && self.access.user.notes
            && self.access.user.files
            && self.access.user.write
            && self.access.groups.len() == 1
            && all_groups.library
            && all_groups.write
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_web_library_access_requires_write_access() {
        let key = |write| {
            serde_json::from_value::<ApiKey>(json!({
                "key": "secret",
                "userID": 1,
                "name": "session-hash",
                "webUI": true,
                "dateAdded": "2026-08-08T00:00:00Z",
                "access": {
                    "user": {"library": true, "notes": true, "files": true, "write": write},
                    "groups": {"all": {"library": true, "write": write}}
                }
            }))
            .expect("valid API key fixture")
        };

        assert!(key(true).has_full_web_library_access());
        assert!(!key(false).has_full_web_library_access());
    }
}
