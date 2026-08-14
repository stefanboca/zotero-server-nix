use anyhow::Context as _;
use serde_json::{Value, json};

use super::Dataserver;
use crate::id::UserId;

#[derive(Clone)]
pub struct LoginSessionInfo {
    pub status: String,
    pub user_id: Option<UserId>,
    pub access: Option<Value>,
}

impl Dataserver {
    pub async fn login_session_info(&self, token: &str) -> anyhow::Result<LoginSessionInfo> {
        let response = self
            .super_request(
                reqwest::Method::GET,
                &format!("/keys/sessions/{token}/info"),
            )
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
            status: value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            user_id: value.get("userID").and_then(Value::as_u64).map(UserId::new),
            access: value
                .get("access")
                .cloned()
                .filter(|value| !value.is_null()),
        })
    }

    pub async fn complete_login_session(
        &self,
        token: &str,
        user_id: UserId,
        access: Option<Value>,
    ) -> anyhow::Result<()> {
        let access = access.unwrap_or_else(|| {
            json!({
                "user": {"library": true, "notes": true, "write": true},
                "groups": {"all": {"library": true, "write": true}}
            })
        });
        self.super_request(reqwest::Method::POST, "/keys/sessions/complete")
            .json(&json!({"sessionToken": token, "userID": user_id, "access": access}))
            .send()
            .await
            .context("completing Zotero login session")?
            .error_for_status()
            .context("Dataserver rejected login-session completion")?;
        Ok(())
    }
}
