// Topcoat composes pages, layouts, and concurrent components into deeply nested
// async view futures. The default rustc query-depth limit is too small to lay
// out the generated route and view types in this application.
#![recursion_limit = "256"]

mod app;
mod config;
mod database;
mod dataserver;
mod id;
mod oidc;
mod provision;

pub use config::Config;
pub use provision::provision;

use std::fmt::Write as _;

use std::{os::unix::fs::PermissionsExt as _, sync::Arc, time::Duration};

use anyhow::Context as _;
use base64::{Engine as _, prelude::BASE64_STANDARD};
use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    cookie::RouterBuilderCookieExt,
    session::{RouterBuilderSessionExt, SessionConfig},
};

use database::Database;
use dataserver::Dataserver;
use oidc::Oidc;

pub async fn serve(config: Arc<Config>) -> anyhow::Result<()> {
    let secrets = config.secrets()?;
    let cookie_key = BASE64_STANDARD
        .decode(secrets.portal_cookie_key.trim())
        .context("decoding PORTAL_COOKIE_KEY")?;
    let cookie_key = topcoat::cookie::Key::try_from(cookie_key.as_slice())
        .context("PORTAL_COOKIE_KEY must decode to at least 64 bytes")?;
    let database = Database::new(config.clone(), secrets.database_url.trim()).await?;
    let dataserver = Dataserver::new(config.clone(), secrets.dataserver_super_password)?;
    let oidc = Oidc::new(config.clone(), secrets.oidc_client_secret).await?;
    let router = app::routes()
        .cookies()
        .sessions(
            SessionConfig::builder()
                .lifetime(Duration::from_secs(config.browser_session_lifetime_seconds))
                .build(),
        )
        .app_context(cookie_key)
        .app_context(database)
        .app_context(dataserver)
        .app_context(oidc)
        .assets(AssetBundle::load().expect("failed to load the Topcoat asset bundle"))
        .build();

    match config.server_address.scheme() {
        "unix" => {
            let socket = config.server_address.path();
            match std::fs::remove_file(&socket) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("removing stale auth socket"),
            };
            let listener =
                tokio::net::UnixListener::bind(&socket).context("binding auth socket")?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))
                .with_context(|| format!("setting portal socket permissions on {socket}"))?;
            tracing::info!(socket, "listening on Unix socket");
            topcoat::serve(listener, router)
                .await
                .context("serving auth portal")
        }
        "http" | "https" => {
            let address = config.server_address.authority();
            let listener = tokio::net::TcpListener::bind(address)
                .await
                .context("binding tcp socket")?;
            tracing::info!(address, "listening on TCP socket");
            topcoat::serve(listener, router)
                .await
                .context("serving auth portal")
        }
        scheme => Err(anyhow::anyhow!("address has unknown scheme `{scheme}`")),
    }
}

fn hex_string(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
