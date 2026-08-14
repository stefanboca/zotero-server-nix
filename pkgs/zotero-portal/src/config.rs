use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::Deserialize;
use url::Url;

const DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_browser_session_lifetime_seconds")]
    pub browser_session_lifetime_seconds: u64,
    pub server_address: Url,
    pub database: DatabaseConfig,
    pub oidc: OidcConfig,
    pub dataserver_url: Url,
    pub stream_server_url: Url,
    pub translation_server_url: Url,
}

pub struct Secrets {
    pub(crate) oidc_client_secret: String,
    pub(crate) dataserver_super_password: String,
}

impl Config {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        toml::from_str(&contents).context("decoding TOML configuration")
    }

    pub fn dev() -> Self {
        Self {
            browser_session_lifetime_seconds: DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS,
            server_address: Url::parse("http://127.0.0.1:3000").unwrap(),
            database: DatabaseConfig {
                url: Url::parse("mysql://development:development@localhost:3306/zotero_www")
                    .unwrap(),
                max_connections: 5,
            },
            oidc: OidcConfig {
                issuer: "http://localhost:9091".into(),
                client_id: "zotero".into(),
                scopes: vec!["profile".into(), "email".into()],
                redirect_url: Url::parse("http://localhost:8080/auth/oidc/callback").unwrap(),
                username_claim: "".into(),
                real_name_claim: "".into(),
                require_verified_email: false,
            },
            dataserver_url: Url::parse("http://localhost:8080/api").unwrap(),
            stream_server_url: Url::parse("ws://localhost:8080/stream/").unwrap(),
            translation_server_url: Url::parse("http://localhost:1969").unwrap(),
        }
    }
}

impl Secrets {
    pub fn from_systemd() -> anyhow::Result<Self> {
        Ok(Self {
            oidc_client_secret: read_credential("oidc-client-secret")?,
            dataserver_super_password: read_credential("dataserver-super-password")?,
        })
    }

    pub fn dev() -> Self {
        Self {
            oidc_client_secret: "development".into(),
            dataserver_super_password: "development".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub url: Url,
    pub max_connections: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect_url: Url,
    pub username_claim: String,
    pub real_name_claim: String,
    pub require_verified_email: bool,
}

fn default_browser_session_lifetime_seconds() -> u64 {
    DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS
}

fn read_credential(name: &str) -> anyhow::Result<String> {
    let directory =
        std::env::var("CREDENTIALS_DIRECTORY").context("CREDENTIALS_DIRECTORY is required")?;
    let value = fs::read_to_string(PathBuf::from(directory).join(name))
        .with_context(|| format!("reading credential {name}"))?;
    Ok(value.trim().to_owned())
}
