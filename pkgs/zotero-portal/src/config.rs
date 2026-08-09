use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::{Deserialize, Deserializer, de};

const DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(
        default = "default_browser_session_lifetime_seconds",
        deserialize_with = "deserialize_browser_session_lifetime_seconds"
    )]
    pub browser_session_lifetime_seconds: u64,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub oidc: OidcConfig,
    pub identity: IdentityConfig,
    pub dataserver: DataserverConfig,
}

impl Config {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        toml::from_str(&contents).context("decoding TOML configuration")
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub socket: PathBuf,
    pub base_uri: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub socket: PathBuf,
    pub user: String,
    pub name: String,
    pub max_connections: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityConfig {
    pub username_claim: String,
    pub real_name_claim: String,
    pub require_verified_email: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataserverConfig {
    pub url: String,
}

pub(crate) fn read_credential(name: &str) -> anyhow::Result<String> {
    let directory =
        env::var("CREDENTIALS_DIRECTORY").context("CREDENTIALS_DIRECTORY is required")?;
    let value = fs::read_to_string(PathBuf::from(directory).join(name))
        .with_context(|| format!("reading credential {name}"))?;
    Ok(value.trim().to_owned())
}

fn default_browser_session_lifetime_seconds() -> u64 {
    DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS
}

fn deserialize_browser_session_lifetime_seconds<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let lifetime = u64::deserialize(deserializer)?;
    if lifetime == 0 {
        return Err(de::Error::custom(
            "browser_session_lifetime_seconds must be greater than zero",
        ));
    }
    Ok(lifetime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_generated_config_shape() {
        let config: Config = toml::from_str(
            r#"
                [server]
                socket = "/run/zotero-portal/zotero-portal.sock"
                base_uri = "zotero.example.com"

                [database]
                socket = "/run/mysqld/mysqld.sock"
                user = "zotero-portal"
                name = "zotero_www"
                max_connections = 10

                [oidc]
                issuer = "https://auth.example.com"
                client_id = "zotero"
                scopes = ["openid", "profile", "email"]

                [identity]
                username_claim = "preferred_username"
                real_name_claim = "name"
                require_verified_email = true

                [dataserver]
                url = "http://localhost"
            "#,
        )
        .expect("valid config");

        assert_eq!(config.database.name, "zotero_www");
        assert_eq!(
            config.browser_session_lifetime_seconds,
            DEFAULT_BROWSER_SESSION_LIFETIME_SECONDS
        );
        assert_eq!(config.oidc.scopes, ["openid", "profile", "email"]);
    }

    #[test]
    fn rejects_a_zero_browser_session_lifetime() {
        let result = toml::from_str::<Config>(
            r#"
                browser_session_lifetime_seconds = 0

                [server]
                socket = "/run/zotero-portal/zotero-portal.sock"
                base_uri = "zotero.example.com"

                [database]
                socket = "/run/mysqld/mysqld.sock"
                user = "zotero-portal"
                name = "zotero_www"
                max_connections = 10

                [oidc]
                issuer = "https://auth.example.com"
                client_id = "zotero"
                scopes = ["openid", "profile", "email"]

                [identity]
                username_claim = "preferred_username"
                real_name_claim = "name"
                require_verified_email = true

                [dataserver]
                url = "http://localhost"
            "#,
        );

        assert!(
            result
                .expect_err("zero lifetime must fail")
                .to_string()
                .contains("browser_session_lifetime_seconds must be greater than zero")
        );
    }

    #[test]
    fn rejects_unknown_config_keys() {
        let result = toml::from_str::<Config>(
            r#"
                unknown = true
            "#,
        );

        assert!(result.is_err());
    }
}
