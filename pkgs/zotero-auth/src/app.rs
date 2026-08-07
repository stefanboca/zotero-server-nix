mod auth;
mod login;

use std::{
    env,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use sqlx::MySqlPool;
use topcoat::{
    Result,
    asset::{Asset, AssetBundle, RouterBuilderAssetExt},
    cookie::RouterBuilderCookieExt,
    router::{RouterBuilder, error::not_found, layout, page},
    session::{RouterBuilderSessionExt, SessionConfig},
    view::view,
};

use crate::{
    config::{Config, read_credential},
    dataserver::Dataserver,
    oidc::Oidc,
    store,
};

const FLOW_LIFETIME: Duration = Duration::from_secs(15 * 60);
const STYLESHEET: Asset = topcoat::asset::asset!("target/topcoat/tailwind.css");

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: MySqlPool,
    pub dataserver: Arc<Dataserver>,
    pub oidc: Arc<Oidc>,
}

pub async fn migrate(config: &Config) -> anyhow::Result<()> {
    let database = store::connect(&config.database)
        .await
        .context("connecting to MariaDB")?;
    sqlx::migrate!()
        .run(&database)
        .await
        .context("running database migrations")
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let database = store::connect(&config.database)
        .await
        .context("connecting to MariaDB")?;
    sqlx::migrate!()
        .run(&database)
        .await
        .context("running database migrations")?;

    let oidc = Oidc::discover(&config, read_credential("oidc-client-secret")?).await?;
    let dataserver = Dataserver::new(&config, read_credential("dataserver-super-password")?)?;
    let socket = config.server.socket.clone();
    let state = AppState {
        config: Arc::new(config),
        database,
        dataserver: Arc::new(dataserver),
        oidc: Arc::new(oidc),
    };
    let router = routes()
        .cookies()
        .sessions(SessionConfig::builder().lifetime(FLOW_LIFETIME).build())
        .app_context(state)
        .build();

    match std::fs::remove_file(&socket) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("removing stale auth socket"),
    }
    let listener = tokio::net::UnixListener::bind(&socket).context("binding auth socket")?;
    topcoat::serve(listener, router)
        .await
        .context("serving auth portal")
}

pub fn routes() -> RouterBuilder {
    topcoat::router::module_router!()
        .assets(AssetBundle::load().expect("failed to load the Topcoat asset bundle"))
}

pub fn is_topcoat_dev() -> bool {
    env::var_os("TOPCOAT_DEV_URL").is_some()
}

pub(crate) fn epoch_seconds() -> anyhow::Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

pub(crate) fn valid_session_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

#[layout]
async fn root_layout(slot: Result) -> Result {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Zotero login"</title>
                <link rel="stylesheet" href=(STYLESHEET)>
                topcoat::dev::script()
            </head>
            <body
                class="grid min-h-screen place-items-center bg-stone-100 p-6 font-sans text-stone-950 dark:bg-stone-950 dark:text-stone-50"
            >
                (slot?)
            </body>
        </html>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_dataserver_session_tokens() {
        assert!(valid_session_token("0123456789abcdefghijklmnopqrstuv"));
        assert!(!valid_session_token("short"));
        assert!(!valid_session_token("0123456789abcdefghijklmnopqr-_"));
    }
}

#[page]
async fn index() -> Result {
    if !is_topcoat_dev() {
        return Err(not_found().into());
    }

    view! {
        <main
            class="w-full max-w-md rounded-xl border border-stone-200 bg-white p-8 shadow-sm dark:border-stone-800 dark:bg-stone-900"
        >
            <h1 class="text-2xl font-semibold">"Zotero auth UI previews"</h1>
            <ul class="mt-4 space-y-2">
                <li>
                    <a
                        class="text-red-800 underline decoration-red-800/40 underline-offset-4 hover:text-red-600 dark:text-red-400 dark:decoration-red-400/40 dark:hover:text-red-300"
                        href="/login"
                    >
                        "Sign in"
                    </a>
                </li>
                <li>
                    <a
                        class="text-red-800 underline decoration-red-800/40 underline-offset-4 hover:text-red-600 dark:text-red-400 dark:decoration-red-400/40 dark:hover:text-red-300"
                        href="/auth/oidc/callback"
                    >
                        "Login complete"
                    </a>
                </li>
            </ul>
        </main>
    }
}
