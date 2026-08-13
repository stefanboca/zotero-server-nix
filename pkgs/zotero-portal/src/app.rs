mod auth;
mod export;
mod groups;
mod import;
mod library;
mod path;
mod search;
mod settings;
mod static_assets;
mod translation;
mod user;
mod users;
mod web;

use std::{
    os::unix::fs::PermissionsExt as _,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use anyhow::Context as _;
use sqlx::{
    ConnectOptions as _, MySqlPool,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, CxBuilder, app_context, try_request_context},
    cookie::RouterBuilderCookieExt,
    router::{
        Body, IntoResponse, Next, Response, RouterBuilder, RouterBuilderDiscoverExt, StatusCode,
        error::{RedirectError, not_found, redirect, redirect_permanent},
        headers, layer, layout, page, uri,
    },
    session::{self, RouterBuilderSessionExt, SessionConfig},
    view::{View, component, view},
};

use crate::{
    app::library::WebLibrary,
    backend::{self, Backend},
    config::{Config, Secrets},
    id::UserId,
};

pub(crate) const FLOW_LIFETIME: Duration = Duration::from_secs(15 * 60);

pub(crate) struct BareLayout;

async fn connect_and_migrate(config: &Config) -> anyhow::Result<MySqlPool> {
    let db = MySqlPoolOptions::new()
        .max_connections(config.database.max_connections)
        .connect_with(MySqlConnectOptions::from_url(&config.database.url)?)
        .await
        .context("connecting to MariaDB")?;
    sqlx::migrate!()
        .run(&db)
        .await
        .context("running database migrations")?;
    Ok(db)
}

pub async fn migrate(config: &Config) -> anyhow::Result<()> {
    connect_and_migrate(config).await?;
    Ok(())
}

pub async fn serve(config: Config, secrets: Secrets) -> anyhow::Result<()> {
    let config = Arc::new(config);
    let db = connect_and_migrate(&config).await?;
    let backend = Backend::new(config.clone(), secrets, db).await?;
    let router = routes()
        .cookies()
        .sessions(
            SessionConfig::builder()
                .lifetime(Duration::from_secs(config.browser_session_lifetime_seconds))
                .build(),
        )
        .app_context(backend)
        .build();

    let address = &config.server.address;
    match address.scheme() {
        "unix" => {
            let socket = address.path();
            match std::fs::remove_file(&socket) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("removing stale auth socket"),
            };
            let listener =
                tokio::net::UnixListener::bind(&socket).context("binding auth socket")?;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))
                .with_context(|| format!("setting portal socket permissions on {socket}"))?;
            topcoat::serve(listener, router)
                .await
                .context("serving auth portal")
        }
        "http" | "https" => {
            let host = address
                .host()
                .map_or_else(|| String::from("127.0.0.1"), |h| h.to_string());
            let port = address.port().unwrap_or(80);
            let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
                .await
                .context("binding tcp socket")?;
            topcoat::serve(listener, router)
                .await
                .context("serving auth portal")
        }
        scheme => Err(anyhow::anyhow!("address has unknown scheme `{scheme}`")),
    }
}

pub fn routes() -> RouterBuilder {
    topcoat::router::module_router!()
        // `module_router!` discovers file-based routes, but runtime procedures
        // are registered through the outer router discovery extension.
        .discover()
        .assets(AssetBundle::load().expect("failed to load the Topcoat asset bundle"))
}

pub(crate) fn valid_session_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

// TODO: clean up this request confirmed nonsense
pub(crate) fn request_confirmed(cx: &Cx) -> bool {
    headers(cx)
        .get("x-zotero-confirmed")
        .is_some_and(|value| value == "true")
}

pub(crate) async fn current_user(cx: &Cx) -> Result<Option<UserId>> {
    let Some(browser_session) = session::refresh(cx).await? else {
        return Ok(None);
    };
    let expires = browser_session
        .expires_at
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    app_context::<Backend>(cx)
        .refresh_browser_session(&*browser_session.token_hash, expires)
        .await
        .map_err(Into::into)
}

pub(crate) async fn authenticated_user(cx: &Cx) -> Result<UserId> {
    if let Some(user_id) = current_user(cx).await? {
        return Ok(user_id);
    }

    let return_path =
        normalized_return_path(uri(cx).path_and_query().map_or("/", |path| path.as_str()))
            .unwrap_or_else(|| "/".to_owned());
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("continue", &return_path)
        .finish();
    Err(redirect(&format!("/user/login?{query}")).into())
}

pub(crate) fn normalized_return_path(value: &str) -> Option<String> {
    if value.len() > 2048 || !value.starts_with('/') {
        return None;
    }

    let base = url::Url::parse("https://zotero.invalid/").expect("static base URL is valid");
    let target = base.join(value).ok()?;
    if target.origin() != base.origin()
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
    {
        return None;
    }

    let mut path = target.path().to_owned();
    if let Some(query) = target.query() {
        path.push('?');
        path.push_str(query);
    }
    Some(path)
}

#[layer]
async fn canonicalize_trailing_slash(
    cx: &mut CxBuilder,
    body: Body,
    next: Next<'_>,
) -> Result<Response> {
    let request_uri = uri(cx);
    let path = request_uri.path();
    if path != "/" && path.ends_with('/') {
        let mut location = path.trim_end_matches('/').to_owned();
        if let Some(query) = request_uri.query() {
            location.push('?');
            location.push_str(query);
        }
        return Err(redirect_permanent(&location).into());
    }
    next.run(cx, body).await
}

#[layout]
async fn root_layout(cx: &Cx, slot: Result) -> Result {
    if headers(cx).get("x-zotero-fragment").is_some() {
        return slot;
    }
    let content = match slot {
        Ok(content) => content,
        Err(error) if error.downcast_ref::<RedirectError>().is_some() => return Err(error),
        Err(error) => {
            let status = error
                .into_response(cx)
                .expect("error.into_response() always return Ok")
                .status();
            friendly_error_page(cx, status).await?
        }
    };
    let web_library_active = try_request_context::<WebLibrary>(cx).is_some();
    let bare_layout = try_request_context::<BareLayout>(cx).is_some();
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Zotero"</title>
                <link rel="icon" href="/static/web-library/icons/32/z.svg">
                <link
                    rel="stylesheet"
                    href=(topcoat::asset::asset!("target/topcoat/tailwind.css"))
                >
                <link
                    rel="stylesheet"
                    href=(topcoat::asset::asset!("src/app/feedback.css"))
                >
                if web_library_active {
                    <link
                        rel="stylesheet"
                        href="/static/web-library/zotero-web-library.css"
                    >
                }
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body
                class="min-h-screen bg-white font-sans text-stone-950 dark:bg-stone-950 dark:text-stone-50"
            >
                if !bare_layout {
                    <header class="border-b border-stone-200 dark:border-stone-800">
                        nav_bar()
                    </header>
                }
                (content)
            </body>
        </html>
    }
}

#[component]
async fn nav_bar(cx: &Cx) -> Result {
    let path = uri(cx).path();
    let navigation_user = navigation_user(cx).await?;
    view! {
        <nav
            class="mx-auto flex w-full max-w-4xl flex-wrap items-center gap-4 px-4 py-4 sm:px-6"
            aria-label="Primary navigation"
        >
            <div class="flex min-w-0 flex-wrap items-center gap-x-5 gap-y-3 sm:gap-x-6">
                let home_active = path == "/";
                <a
                    class=(if home_active {
                        "border-b-2 border-red-800 pb-1 text-lg font-semibold text-red-800 dark:border-red-400 dark:text-red-400"
                    } else {
                        "border-b-2 border-transparent pb-1 text-lg font-semibold text-red-800 dark:text-red-400"
                    })
                    href="/"
                    aria-current=(home_active.then_some("page"))
                >
                    "Zotero"
                </a>
                for (route, name) in [("/groups", "Groups"), ("/settings", "Settings")] {
                    let active = path == route
                        || path.starts_with(&format!("{route}/"));
                    <a
                        class=(if active {
                            "border-b-2 border-red-800 pb-1 text-sm font-semibold text-red-800 dark:border-red-400 dark:text-red-400"
                        } else {
                            "border-b-2 border-transparent pb-1 text-sm hover:text-red-800 dark:hover:text-red-400"
                        })
                        href=(route)
                        aria-current=(active.then_some("page"))
                    >
                        (name)
                    </a>
                }
            </div>
            <div
                class="flex w-full min-w-0 items-center justify-between gap-3 sm:ml-auto sm:w-auto"
            >
                if let Some(user) = &navigation_user {
                    <span
                        class="min-w-0 truncate text-sm text-stone-600 dark:text-stone-400"
                    >
                        "Signed in as "
                        (user.display_name())
                    </span>
                    <form method="post" action="/user/logout">
                        <button
                            class="border border-stone-300 px-3 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                            type="submit"
                        >
                            "Log out"
                        </button>
                    </form>
                }
            </div>
        </nav>
    }
}

async fn navigation_user(cx: &Cx) -> Result<Option<backend::UserSummary>> {
    let Some(user_id) = current_user(cx).await? else {
        return Ok(None);
    };
    app_context::<Backend>(cx)
        .user_by_id(user_id)
        .await
        .map_err(Into::into)
}

async fn friendly_error_page(cx: &Cx, status: StatusCode) -> Result<View> {
    let message = match status {
        StatusCode::UNAUTHORIZED => "Sign in and try again",
        StatusCode::FORBIDDEN => "Your account is not allowed to perform that action.",
        StatusCode::NOT_FOUND => "The page may have moved, or you may not have access to it.",
        _ => "The request could not be completed. Please try again",
    };
    view! {
        cx =>
        (status)
        <main class="mx-auto w-full max-w-4xl px-4 py-12 sm:px-6 sm:py-16">
            <h1 class="text-4xl font-semibold text-red-800 dark:text-red-400">
                (status.as_u16())
            </h1>
            if let Some(title) = status.canonical_reason() {
                <p class="mt-2 text-2xl font-semibold">(title)</p>
            }
            <p class="mt-4 max-w-2xl leading-7 text-stone-600 dark:text-stone-400">
                (message)
            </p>
        </main>
    }
}

pub(crate) async fn signed_in_user(cx: &Cx) -> Result<backend::UserSummary> {
    let user_id = authenticated_user(cx).await?;
    app_context::<Backend>(cx)
        .user_by_id(user_id)
        .await?
        .ok_or_else(|| not_found().into())
}

#[component]
pub(crate) async fn username_label(username: &str) -> Result {
    view! { <span class="font-mono font-semibold">(username)</span> }
}

#[component]
pub(crate) async fn toast_script() -> Result {
    view! {
        <script type="module" src=(topcoat::asset::asset!("src/app/toast.js"))></script>
    }
}

#[component]
pub(crate) async fn confirmation_dialog() -> Result {
    view! {
        <dialog
            class="fixed inset-0 m-auto max-h-[calc(100vh_-_2rem)] w-[calc(100%_-_2rem)] max-w-lg overflow-y-auto border border-stone-300 bg-white p-0 text-stone-950 shadow-xl backdrop:bg-stone-950/60 dark:border-stone-700 dark:bg-stone-950 dark:text-stone-50"
            data-confirm-dialog=""
        >
            <div class="p-6">
                <h2 class="text-xl font-semibold" data-confirm-title="">
                    "Confirm action"
                </h2>
                <p
                    class="mt-3 leading-7 text-stone-600 dark:text-stone-400"
                    data-confirm-message=""
                ></p>
                <div class="mt-5" data-confirm-input-container="" hidden="hidden">
                    <label
                        class="block font-medium"
                        data-confirm-input-label=""
                        for="portal-confirmation"
                    ></label>
                    <input
                        class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        id="portal-confirmation"
                        autocomplete="off"
                        data-confirm-input=""
                    >
                    <p
                        class="mt-2 text-sm text-stone-600 dark:text-stone-400"
                        data-confirm-input-help=""
                    ></p>
                </div>
                <div class="mt-6 flex flex-wrap justify-end gap-3">
                    <button
                        class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-stone-700 dark:hover:bg-stone-800"
                        type="button"
                        data-confirm-submit=""
                    >
                        "Confirm"
                    </button>
                    <button
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                        type="button"
                        data-confirm-cancel=""
                    >
                        "Cancel"
                    </button>
                </div>
            </div>
        </dialog>
        <script type="module" src=(topcoat::asset::asset!("src/app/confirmation.js"))></script>
    }
}

#[page]
async fn index(cx: &Cx) -> Result {
    authenticated_user(cx).await?;
    view! {
        <main class="w-full bg-white text-stone-950">
            <iframe
                class="block h-[calc(100dvh-4.75rem)] min-h-[32rem] w-full border-0"
                src="/library"
                title="Zotero library"
            ></iframe>
        </main>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_tree_builds_without_duplicate_routes_or_assets() {
        let _router = routes().build();
    }

    #[test]
    fn accepts_only_dataserver_session_tokens() {
        assert!(valid_session_token("0123456789abcdefghijklmnopqrstuv"));
        assert!(!valid_session_token("short"));
        assert!(!valid_session_token("0123456789abcdefghijklmnopqr-_"));
    }

    #[test]
    fn normalizes_only_same_origin_return_paths() {
        assert_eq!(
            normalized_return_path("/groups/../groups/new?from=desktop"),
            Some("/groups/new?from=desktop".to_owned())
        );
        assert_eq!(normalized_return_path("/"), Some("/".to_owned()));
        assert_eq!(normalized_return_path("https://example.com/groups"), None);
        assert_eq!(normalized_return_path("//example.com/groups"), None);
        assert_eq!(normalized_return_path("/\\example.com/groups"), None);
        assert_eq!(normalized_return_path("/groups#fragment"), None);
    }

    #[test]
    fn browser_mutations_use_topcoat_form_encoding() {
        let confirmation = include_str!("app/confirmation.js");
        let group_actions = include_str!("app/groups/group_id/group_actions.js");
        assert!(confirmation.contains("new URLSearchParams(new FormData(form))"));
        assert!(!group_actions.contains("fetch("));
        assert!(!group_actions.contains("FormData"));
        assert!(!group_actions.contains("createContextualFragment"));
    }
}
