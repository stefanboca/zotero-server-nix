use std::{env, fs, path::Path, sync::Arc};

use anyhow::Context as _;
use serde::Deserialize;
use url::Url;

secretspec_derive::declare_secrets!("secretspec.toml");

const DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip, default = "production_profile")]
    profile: Profile,
    #[serde(default = "default_browser_session_lifetime_seconds")]
    pub browser_session_lifetime_seconds: u64,
    pub server_address: Url,
    #[serde(default)]
    pub database: DatabaseConfig,
    pub oidc: OidcConfig,
    pub dataserver_url: Url,
    pub stream_server_url: Url,
    pub translation_server_url: Url,
}

impl Config {
    pub fn new(path: Option<&Path>) -> anyhow::Result<Arc<Self>> {
        if cfg!(debug_assertions) && env::var_os("TOPCOAT_DEV_URL").is_some() {
            return Ok(Self::development());
        }

        let path = path.context("--config is required outside development mode")?;
        let contents = fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        toml::from_str(&contents)
            .map(Arc::new)
            .context("decoding TOML configuration")
    }

    fn development() -> Arc<Self> {
        Arc::new(Self {
            profile: Profile::Development,
            browser_session_lifetime_seconds: DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS,
            server_address: Url::parse("http://127.0.0.1:3000").unwrap(),
            database: DatabaseConfig { max_connections: 5 },
            oidc: OidcConfig {
                issuer: "http://localhost:9091".into(),
                client_id: "zotero".into(),
                redirect_url: Url::parse("http://localhost:8080/auth/oidc/callback").unwrap(),
                require_verified_email: false,
            },
            dataserver_url: Url::parse("http://localhost:8080/api").unwrap(),
            stream_server_url: Url::parse("ws://localhost:8080/stream/").unwrap(),
            translation_server_url: Url::parse("http://localhost:1969").unwrap(),
        })
    }

    pub fn secrets(&self) -> anyhow::Result<SecretSpec> {
        Ok(SecretSpec::builder()
            .with_profile(self.profile)
            .with_reason("start zotero-portal")
            .load()
            .context("resolving Zotero service secrets")?
            .secrets)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_max_connections")]
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: default_database_max_connections(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub redirect_url: Url,
    #[serde(default = "default_require_verified_email")]
    pub require_verified_email: bool,
}

fn default_browser_session_lifetime_seconds() -> u64 {
    DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS
}

fn production_profile() -> Profile {
    Profile::Production
}

fn default_database_max_connections() -> u32 {
    5
}

fn default_require_verified_email() -> bool {
    true
}
