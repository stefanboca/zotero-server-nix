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
            read_only: false,
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
        let keys = self.api_keys(user_id).await?;
        if let Some(key) = keys
            .iter()
            .find(|key| key.user_id == user_id && key.web_ui && key.name == session_handle)
        {
            return Ok(key.key.clone());
        }

        let response = self
            .super_request(reqwest::Method::POST, &format!("/users/{user_id}/keys"))
            .json(&json!({
                "name": session_handle,
                "webUI": true,
                "access": {
                    "user": {"library": true, "notes": true, "write": true},
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

    async fn delete_api_key(&self, key: &str) -> anyhow::Result<()> {
        self.super_request(reqwest::Method::DELETE, &format!("/keys/{key}"))
            .send()
            .await
            .context("removing a Zotero API key")?
            .error_for_status()
            .context("Dataserver rejected Zotero API key removal")?;
        Ok(())
    }

    pub async fn connected_clients(&self, user_id: UserId) -> anyhow::Result<Vec<ConnectedClient>> {
        Ok(self
            .api_keys(user_id)
            .await?
            .into_iter()
            .filter(|key| !key.web_ui)
            .map(|key| ConnectedClient {
                handle: secret_handle(&key.key),
                name: key.name,
                date_added: key.date_added,
                last_used: key.last_used,
            })
            .collect())
    }

    pub async fn revoke_connected_client(
        &self,
        user_id: UserId,
        handle: &str,
    ) -> anyhow::Result<bool> {
        if handle.len() != 64 || !handle.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(false);
        }
        let Some(key) =
            self.api_keys(user_id).await?.into_iter().find(|key| {
                key.user_id == user_id && !key.web_ui && secret_handle(&key.key) == handle
            })
        else {
            return Ok(false);
        };
        self.delete_api_key(&key.key).await?;
        Ok(true)
    }

    pub async fn revoke_all_connected_clients(&self, user_id: UserId) -> anyhow::Result<usize> {
        let keys = self.api_keys(user_id).await?;
        let mut revoked = 0;
        for key in keys
            .into_iter()
            .filter(|key| key.user_id == user_id && !key.web_ui)
        {
            self.delete_api_key(&key.key).await?;
            revoked += 1;
        }
        Ok(revoked)
    }

    pub async fn revoke_web_library_api_keys(
        &self,
        user_id: UserId,
        session_handles: &[String],
    ) -> anyhow::Result<()> {
        if session_handles.is_empty() {
            return Ok(());
        }
        let keys = self.api_keys(user_id).await?;
        for key in keys.into_iter().filter(|key| {
            key.user_id == user_id
                && key.web_ui
                && session_handles.iter().any(|handle| handle == &key.name)
        }) {
            self.delete_api_key(&key.key).await?;
        }
        Ok(())
    }
}

impl ApiKey {
    fn has_full_web_library_access(&self) -> bool {
        let Some(all_groups) = self.access.groups.get("all").and_then(Value::as_object) else {
            return false;
        };
        self.access.user.library
            && self.access.user.notes
            && self.access.user.write
            && self.access.groups.len() == 1
            && all_groups.get("library").and_then(Value::as_bool) == Some(true)
            && all_groups.get("write").and_then(Value::as_bool) == Some(true)
    }
}

fn secret_handle(secret: &str) -> String {
    hex_string(&Sha256::digest(secret.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataserver::{groups::xml_escape, storage::xml_number};

    #[test]
    fn escapes_group_names_for_xml_attributes() {
        assert_eq!(
            xml_escape("Research & <notes> \"team\"'s"),
            "Research &amp; &lt;notes&gt; &quot;team&quot;&apos;s"
        );
    }

    #[test]
    fn group_user_ids_include_each_person_once() {
        let group = Group {
            id: 1,
            name: "Research".to_owned(),
            owner: UserId::new(1),
            admins: vec![UserId::new(1), UserId::new(2)],
            members: vec![UserId::new(1), UserId::new(2), UserId::new(3)],
            group_type: "Private".to_owned(),
            library_editing: "members".to_owned(),
            library_reading: "members".to_owned(),
            file_editing: "members".to_owned(),
            description: String::new(),
            url: String::new(),
            has_image: false,
        };

        assert_eq!(
            group.user_ids(),
            [UserId::new(1), UserId::new(2), UserId::new(3)]
        );
    }

    #[test]
    fn parses_storage_usage_numbers() {
        let response =
            "<storage><usage><total>1536.5</total><library>512</library></usage></storage>";
        assert_eq!(xml_number(response, "total"), Some(1536.5));
        assert_eq!(xml_number(response, "library"), Some(512.0));
        assert_eq!(xml_number(response, "missing"), None);
    }

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
                    "user": {"library": true, "notes": true, "write": write},
                    "groups": {"all": {"library": true, "write": write}}
                }
            }))
            .expect("valid API key fixture")
        };

        assert!(key(true).has_full_web_library_access());
        assert!(!key(false).has_full_web_library_access());
    }

    #[test]
    fn group_role_matches_topcoat_browser_round_trip() {
        let serialized =
            serde_json::to_string(&GroupRole::Admin).expect("group role must serialize");
        assert_eq!(serialized, r#"{"t":"str","v":"admin"}"#);

        let deserialized: GroupRole =
            serde_json::from_str(r#""member""#).expect("browser role must deserialize");
        assert_eq!(deserialized, GroupRole::Member);
    }
}
