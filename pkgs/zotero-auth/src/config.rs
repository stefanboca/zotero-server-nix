use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_generated_config_shape() {
        let config: Config = toml::from_str(
            r#"
                [server]
                socket = "/run/zotero-auth/zotero-auth.sock"
                base_uri = "zotero.example.com"

                [database]
                socket = "/run/mysqld/mysqld.sock"
                user = "zotero-auth"
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
        assert_eq!(config.oidc.scopes, ["openid", "profile", "email"]);
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
