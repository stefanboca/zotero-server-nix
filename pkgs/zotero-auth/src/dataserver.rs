use anyhow::Context as _;
use serde_json::{Value, json};

use crate::{config::Config, oidc::value_string};

pub struct Dataserver {
    http_client: reqwest::Client,
    super_password: String,
    url: String,
}

pub struct LoginSessionInfo {
    pub status: String,
    pub user_id: Option<u64>,
    pub access: Option<Value>,
}

impl Dataserver {
    pub fn new(config: &Config, super_password: String) -> anyhow::Result<Self> {
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("building Dataserver HTTP client")?;
        Ok(Self {
            http_client,
            super_password,
            url: config.dataserver.url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn login_session_info(&self, token: &str) -> anyhow::Result<LoginSessionInfo> {
        let response = self
            .http_client
            .get(format!("{}/keys/sessions/{token}/info", self.url))
            .basic_auth("zotero-auth", Some(&self.super_password))
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

    pub async fn complete_login_session(
        &self,
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
        self.http_client
            .post(format!("{}/keys/sessions/complete", self.url))
            .basic_auth("zotero-auth", Some(&self.super_password))
            .json(&json!({"sessionToken": token, "userID": user_id, "access": access}))
            .send()
            .await
            .context("completing Zotero login session")?
            .error_for_status()
            .context("Dataserver rejected login-session completion")?;
        Ok(())
    }

    pub async fn set_unlimited_storage(&self, user_id: u64) -> anyhow::Result<()> {
        self.http_client
            .post(format!("{}/users/{user_id}/storageadmin", self.url))
            .basic_auth("zotero-auth", Some(&self.super_password))
            .form(&[("quota", "unlimited"), ("expiration", "0")])
            .send()
            .await
            .context("setting unlimited Zotero file storage")?
            .error_for_status()
            .context("Dataserver rejected unlimited file storage")?;
        Ok(())
    }

    pub async fn create_private_group(&self, user_id: u64, name: &str) -> anyhow::Result<()> {
        let name = xml_escape(name);
        let body = format!(
            r#"<group owner="{user_id}" name="{name}" type="Private" libraryEditing="members" libraryReading="members" fileEditing="members" hasImage="0"><description></description><url></url></group>"#
        );
        self.http_client
            .post(format!("{}/groups", self.url))
            .basic_auth("zotero-auth", Some(&self.super_password))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("creating Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group creation")?;
        Ok(())
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_group_names_for_xml_attributes() {
        assert_eq!(
            xml_escape("Research & <notes> \"team\"'s"),
            "Research &amp; &lt;notes&gt; &quot;team&quot;&apos;s"
        );
    }
}
