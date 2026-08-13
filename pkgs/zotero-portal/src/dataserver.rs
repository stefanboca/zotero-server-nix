use anyhow::Context as _;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use topcoat::runtime::{BoolSurrogate, Surrogate, Surrogated};

use crate::{config::Config, hex_string, id::UserId, oidc::value_string};

pub struct Dataserver {
    http_client: reqwest::Client,
    super_password: String,
    url: String,
}

const WEB_LIBRARY_KEY_NAME: &str = "Zotero Web Library (this portal)";

#[derive(Clone)]
pub struct LoginSessionInfo {
    pub status: String,
    pub user_id: Option<UserId>,
    pub access: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ConnectedClient {
    pub handle: String,
    pub name: String,
    pub date_added: String,
    pub last_used: Option<String>,
}

#[derive(Deserialize)]
struct ApiKey {
    key: String,
    #[serde(rename = "userID")]
    user_id: UserId,
    name: String,
    #[serde(rename = "dateAdded")]
    date_added: String,
    #[serde(default, rename = "lastUsed")]
    last_used: Option<String>,
    #[serde(default)]
    access: ApiKeyAccess,
}

#[derive(Default, Deserialize)]
struct ApiKeyAccess {
    #[serde(default)]
    user: ApiKeyLibraryAccess,
    #[serde(default)]
    groups: Map<String, Value>,
}

#[derive(Default, Deserialize)]
struct ApiKeyLibraryAccess {
    #[serde(default)]
    library: bool,
    #[serde(default)]
    notes: bool,
    #[serde(default)]
    write: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct StorageUsage {
    pub total_mb: f64,
    pub library_mb: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(try_from = "String")]
pub enum GroupRole {
    Owner,
    Admin,
    Member,
}

impl GroupRole {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    pub fn can_manage(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Owner => "Group owner",
            Self::Admin => "Administrator",
            Self::Member => "Member",
        }
    }

    pub fn user_summary(self) -> &'static str {
        match self {
            Self::Owner => "You are the owner",
            Self::Admin => "You are an administrator",
            Self::Member => "You are a member",
        }
    }

    pub fn eq(&self, other: &Self) -> BoolSurrogate {
        (*self == *other).into_surrogate()
    }

    pub fn ne(&self, other: &Self) -> BoolSurrogate {
        (*self != *other).into_surrogate()
    }
}

impl std::fmt::Display for GroupRole {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.wire_name())
    }
}

impl std::str::FromStr for GroupRole {
    type Err = &'static str;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err("unknown group role"),
        }
    }
}

impl TryFrom<String> for GroupRole {
    type Error = &'static str;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for GroupRole {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct TaggedRole<'a> {
            t: &'static str,
            v: &'a str,
        }

        TaggedRole {
            t: "str",
            v: self.wire_name(),
        }
        .serialize(serializer)
    }
}

impl Surrogated for GroupRole {
    type Surrogate = Self;

    fn into_surrogate(self) -> Self::Surrogate {
        self
    }
}

impl Surrogate for GroupRole {
    type Real = GroupRole;

    fn into_real(self) -> Self::Real {
        self
    }
}

impl Surrogated for &GroupRole {
    type Surrogate = Self;

    fn into_surrogate(self) -> Self::Surrogate {
        self
    }
}

impl Surrogate for &GroupRole {
    type Real = Self;

    fn into_real(self) -> Self::Real {
        self
    }
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub owner: UserId,
    pub admins: Vec<UserId>,
    pub members: Vec<UserId>,
    pub group_type: String,
    pub library_editing: String,
    pub library_reading: String,
    pub file_editing: String,
    pub description: String,
    pub url: String,
    pub has_image: bool,
}

impl Group {
    pub fn role(&self, user_id: UserId) -> Option<GroupRole> {
        if self.owner == user_id {
            Some(GroupRole::Owner)
        } else if self.admins.contains(&user_id) {
            Some(GroupRole::Admin)
        } else if self.members.contains(&user_id) {
            Some(GroupRole::Member)
        } else {
            None
        }
    }

    pub fn user_ids(&self) -> Vec<UserId> {
        let mut ids = Vec::with_capacity(1 + self.admins.len() + self.members.len());
        for user_id in std::iter::once(&self.owner)
            .chain(&self.admins)
            .chain(&self.members)
        {
            if !ids.contains(user_id) {
                ids.push(*user_id);
            }
        }
        ids
    }
}

#[derive(Deserialize)]
struct GroupEnvelope {
    id: u64,
    data: GroupData,
}

#[derive(Deserialize)]
struct GroupData {
    name: String,
    owner: UserId,
    #[serde(default)]
    admins: Vec<UserId>,
    #[serde(default)]
    members: Vec<UserId>,
    #[serde(rename = "type")]
    group_type: String,
    #[serde(rename = "libraryEditing")]
    library_editing: String,
    #[serde(rename = "libraryReading")]
    library_reading: String,
    #[serde(rename = "fileEditing")]
    file_editing: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "hasImage")]
    has_image: u8,
}

impl From<GroupEnvelope> for Group {
    fn from(value: GroupEnvelope) -> Self {
        Self {
            id: value.id,
            name: value.data.name,
            owner: value.data.owner,
            admins: value.data.admins,
            members: value.data.members,
            group_type: value.data.group_type,
            library_editing: value.data.library_editing,
            library_reading: value.data.library_reading,
            file_editing: value.data.file_editing,
            description: value.data.description,
            url: value.data.url,
            has_image: value.data.has_image != 0,
        }
    }
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
            .basic_auth("zotero-portal", Some(&self.super_password))
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
        self.http_client
            .post(format!("{}/keys/sessions/complete", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .json(&json!({"sessionToken": token, "userID": user_id, "access": access}))
            .send()
            .await
            .context("completing Zotero login session")?
            .error_for_status()
            .context("Dataserver rejected login-session completion")?;
        Ok(())
    }

    pub async fn set_unlimited_storage(&self, user_id: UserId) -> anyhow::Result<()> {
        self.http_client
            .post(format!("{}/users/{user_id}/storageadmin", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .form(&[("quota", "unlimited"), ("expiration", "0")])
            .send()
            .await
            .context("setting unlimited Zotero file storage")?
            .error_for_status()
            .context("Dataserver rejected unlimited file storage")?;
        Ok(())
    }

    pub async fn storage_usage(&self, user_id: UserId) -> anyhow::Result<StorageUsage> {
        let response = self
            .http_client
            .get(format!("{}/users/{user_id}/storageadmin", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .send()
            .await
            .context("requesting Zotero storage usage")?
            .error_for_status()
            .context("Dataserver rejected storage usage request")?;
        let body = response
            .text()
            .await
            .context("reading Zotero storage usage response")?;
        Ok(StorageUsage {
            total_mb: xml_number(&body, "total")
                .context("Dataserver storage response omitted total usage")?,
            library_mb: xml_number(&body, "library")
                .context("Dataserver storage response omitted library usage")?,
        })
    }

    async fn api_keys(&self, user_id: UserId) -> anyhow::Result<Vec<ApiKey>> {
        let response = self
            .http_client
            .get(format!("{}/users/{user_id}/keys", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Zotero-API-Version", "3")
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

    pub async fn web_library_api_key(&self, user_id: UserId) -> anyhow::Result<String> {
        let keys = self.api_keys(user_id).await?;
        if let Some(key) = keys.iter().find(|key| {
            key.user_id == user_id && key.name == WEB_LIBRARY_KEY_NAME && key.is_web_library_key()
        }) {
            return Ok(key.key.clone());
        }

        for key in keys
            .iter()
            .filter(|key| key.user_id == user_id && key.name == WEB_LIBRARY_KEY_NAME)
        {
            self.delete_api_key(&key.key).await?;
        }

        let response = self
            .http_client
            .post(format!("{}/users/{user_id}/keys", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Zotero-API-Version", "3")
            .json(&json!({
                "name": WEB_LIBRARY_KEY_NAME,
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
        if key.user_id != user_id || !key.is_web_library_key() {
            anyhow::bail!("Dataserver created an invalid Zotero Web Library API key");
        }
        Ok(key.key)
    }

    async fn delete_api_key(&self, key: &str) -> anyhow::Result<()> {
        self.http_client
            .delete(format!("{}/keys/{key}", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Zotero-API-Version", "3")
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
        let Some(key) = self
            .api_keys(user_id)
            .await?
            .into_iter()
            .find(|key| key.user_id == user_id && secret_handle(&key.key) == handle)
        else {
            return Ok(false);
        };
        self.delete_api_key(&key.key).await?;
        Ok(true)
    }

    pub async fn revoke_all_connected_clients(&self, user_id: UserId) -> anyhow::Result<usize> {
        let keys = self.api_keys(user_id).await?;
        let mut revoked = 0;
        for key in keys.into_iter().filter(|key| key.user_id == user_id) {
            self.delete_api_key(&key.key).await?;
            revoked += 1;
        }
        Ok(revoked)
    }

    pub async fn groups_for_user(&self, user_id: UserId) -> anyhow::Result<Vec<Group>> {
        let response = self
            .http_client
            .get(format!("{}/users/{user_id}/groups", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .query(&[("limit", "100")])
            .send()
            .await
            .context("listing Zotero groups")?
            .error_for_status()
            .context("Dataserver rejected group listing")?;
        let groups: Vec<GroupEnvelope> = response
            .json()
            .await
            .context("decoding Zotero group listing")?;
        Ok(groups.into_iter().map(Into::into).collect())
    }

    pub async fn group(&self, group_id: u64) -> anyhow::Result<Option<Group>> {
        let response = self
            .http_client
            .get(format!("{}/groups/{group_id}", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .send()
            .await
            .context("loading Zotero group")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .context("Dataserver rejected group lookup")?;
        let group: GroupEnvelope = response.json().await.context("decoding Zotero group")?;
        Ok(Some(group.into()))
    }

    pub async fn create_private_group(&self, user_id: UserId, name: &str) -> anyhow::Result<u64> {
        let name = xml_escape(name);
        let body = format!(
            r#"<group owner="{user_id}" name="{name}" type="Private" libraryEditing="members" libraryReading="members" fileEditing="members" hasImage="0"><description></description><url></url></group>"#
        );
        let response = self
            .http_client
            .post(format!("{}/groups", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("creating Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group creation")?;
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .context("Dataserver omitted created group location")?
            .to_str()
            .context("Dataserver returned an invalid group location")?;
        location
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .context("Dataserver returned an empty group location")?
            .parse()
            .context("Dataserver returned a group location without a numeric ID")
    }

    pub async fn set_group_user(
        &self,
        group_id: u64,
        user_id: UserId,
        role: GroupRole,
    ) -> anyhow::Result<()> {
        let body = format!(r#"<user id="{user_id}" role="{role}"/>"#);
        self.http_client
            .put(format!("{}/groups/{group_id}/users/{user_id}", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("updating Zotero group member")?
            .error_for_status()
            .context("Dataserver rejected group member update")?;
        Ok(())
    }

    pub async fn rename_group(&self, group: &Group, name: &str) -> anyhow::Result<()> {
        let body = format!(
            r#"<group id="{}" owner="{}" name="{}" type="{}" libraryEditing="{}" libraryReading="{}" fileEditing="{}" hasImage="{}"><description>{}</description><url>{}</url></group>"#,
            group.id,
            group.owner,
            xml_escape(name),
            xml_escape(&group.group_type),
            xml_escape(&group.library_editing),
            xml_escape(&group.library_reading),
            xml_escape(&group.file_editing),
            u8::from(group.has_image),
            xml_escape(&group.description),
            xml_escape(&group.url),
        );
        self.http_client
            .put(format!("{}/groups/{}", self.url, group.id))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Content-Type", "text/xml")
            .body(body)
            .send()
            .await
            .context("renaming Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group rename")?;
        Ok(())
    }

    pub async fn remove_group_user(&self, group_id: u64, user_id: UserId) -> anyhow::Result<()> {
        self.http_client
            .delete(format!("{}/groups/{group_id}/users/{user_id}", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .send()
            .await
            .context("removing Zotero group member")?
            .error_for_status()
            .context("Dataserver rejected group member removal")?;
        Ok(())
    }

    pub async fn delete_group(&self, group_id: u64) -> anyhow::Result<()> {
        self.http_client
            .delete(format!("{}/groups/{group_id}", self.url))
            .basic_auth("zotero-portal", Some(&self.super_password))
            .send()
            .await
            .context("deleting Zotero group")?
            .error_for_status()
            .context("Dataserver rejected group deletion")?;
        Ok(())
    }
}

impl ApiKey {
    fn is_web_library_key(&self) -> bool {
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

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn secret_handle(secret: &str) -> String {
    hex_string(&Sha256::digest(secret.as_bytes()))
}

fn xml_number(value: &str, element: &str) -> Option<f64> {
    let opening = format!("<{element}>");
    let closing = format!("</{element}>");
    let start = value.find(&opening)? + opening.len();
    let end = value[start..].find(&closing)? + start;
    value[start..end].trim().parse().ok()
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
    fn web_library_key_requires_write_access() {
        let key = |write| {
            serde_json::from_value::<ApiKey>(json!({
                "key": "secret",
                "userID": 1,
                "name": WEB_LIBRARY_KEY_NAME,
                "dateAdded": "2026-08-08T00:00:00Z",
                "access": {
                    "user": {"library": true, "notes": true, "write": write},
                    "groups": {"all": {"library": true, "write": write}}
                }
            }))
            .expect("valid API key fixture")
        };

        assert!(key(true).is_web_library_key());
        assert!(!key(false).is_web_library_key());
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
