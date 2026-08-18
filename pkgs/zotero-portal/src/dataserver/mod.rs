mod groups;
mod keys;
mod login;
mod storage;
mod translation;

pub(crate) use groups::group_slug;
pub use translation::{TranslationEndpoint, TranslationResponse};

use std::{collections::BTreeMap, sync::Arc};

use anyhow::Context as _;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;

use crate::{config::Config, id::UserId};

pub struct Dataserver {
    config: Arc<Config>,
    client: reqwest::Client,
    super_password: String,
}

pub struct WebLibraryAccess {
    pub api_key: String,
    pub api_url: url::Url,
    pub read_only: bool,
    pub streaming_url: url::Url,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApiKey {
    pub key: String,
    #[serde(rename = "userID")]
    pub user_id: UserId,
    pub name: String,
    #[serde(default, rename = "webUI")]
    pub web_ui: bool,
    #[serde(default, rename = "dateAdded")]
    pub date_added: String,
    #[serde(default, rename = "lastUsed")]
    pub last_used: Option<String>,
    #[serde(default, rename = "recentIPs")]
    pub recent_ips: Vec<String>,
    #[serde(default)]
    pub access: ApiKeyAccess,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyAccess {
    #[serde(default)]
    pub user: ApiKeyLibraryAccess,
    #[serde(default)]
    pub groups: BTreeMap<ApiKeyGroupTarget, ApiKeyGroupAccess>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ApiKeyGroupTarget {
    All,
    Group(u64),
}

impl Serialize for ApiKeyGroupTarget {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::All => serializer.serialize_str("all"),
            Self::Group(group_id) => serializer.serialize_str(&group_id.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for ApiKeyGroupTarget {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "all" {
            return Ok(Self::All);
        }
        let group_id = value.parse::<u64>().map_err(serde::de::Error::custom)?;
        if group_id == 0 {
            return Err(serde::de::Error::custom("group ID must be nonzero"));
        }
        Ok(Self::Group(group_id))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyLibraryAccess {
    #[serde(default)]
    pub library: bool,
    #[serde(default)]
    pub notes: bool,
    #[serde(default)]
    pub files: bool,
    #[serde(default)]
    pub write: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApiKeyGroupAccess {
    #[serde(default)]
    pub library: bool,
    #[serde(default)]
    pub write: bool,
}

#[derive(Clone, Debug)]
pub struct StorageUsage {
    pub quota_mb: Option<f64>,
    pub expiration: Option<jiff::Timestamp>,
    pub total_mb: f64,
    pub library_mb: f64,
    pub groups: Vec<StorageGroupUsage>,
}

#[derive(Clone, Copy, Debug)]
pub struct StorageGroupUsage {
    pub group_id: u64,
    pub usage_mb: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupRole {
    Owner,
    Admin,
    Member,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub created: jiff::Timestamp,
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

#[derive(Clone, Debug)]
pub struct GroupItemSummary {
    pub key: String,
    pub title: String,
    pub added_by: Option<String>,
    pub date_modified: jiff::Timestamp,
}

#[derive(Clone, Debug)]
pub struct RecentGroupItems {
    pub items: Vec<GroupItemSummary>,
    pub total: u64,
}

#[derive(Deserialize)]
struct GroupEnvelope {
    id: u64,
    meta: GroupMeta,
    data: GroupData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupMeta {
    created: jiff::Timestamp,
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

impl Dataserver {
    pub fn new(config: Arc<Config>, super_password: String) -> anyhow::Result<Self> {
        let client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("building Dataserver HTTP client")?;
        Ok(Self {
            config,
            client,
            super_password,
        })
    }

    fn super_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(
                method,
                format!(
                    "{}{}",
                    self.config.dataserver_url.as_str().trim_end_matches('/'),
                    path
                ),
            )
            .basic_auth("zotero-portal", Some(&self.super_password))
            .header("Zotero-API-Version", "3")
    }
}
