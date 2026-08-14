mod groups;
mod keys;
mod login;
mod storage;
mod translation;

pub use translation::{TranslationEndpoint, TranslationResponse};

use std::sync::Arc;

use anyhow::Context as _;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use topcoat::runtime::{BoolSurrogate, Surrogate, Surrogated};

use crate::{config::Config, hex_string, id::UserId};

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
    #[serde(default, rename = "webUI")]
    web_ui: bool,
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
