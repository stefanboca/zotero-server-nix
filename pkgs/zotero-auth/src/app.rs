mod account;
mod auth;
mod groups;
mod settings;
mod user;
mod users;

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::{Cx, CxBuilder, app_context, try_request_context},
    cookie::RouterBuilderCookieExt,
    router::{
        Body, Next, Response, RouterBuilder, RouterBuilderDiscoverExt, StatusCode,
        error::{
            BadRequestError, ForbiddenError, InternalServerError, MethodNotAllowedError,
            NotFoundError, RedirectError, UnauthorizedError, not_found, redirect,
            redirect_permanent,
        },
        headers, layer, layout, page, uri,
    },
    session::{self, RouterBuilderSessionExt, SessionConfig},
    view::{component, view},
};

use crate::{
    backend::Backend,
    config::{Config, read_credential},
    dataserver::Dataserver,
    id::UserId,
    oidc::Oidc,
    store,
};

pub(crate) const FLOW_LIFETIME: Duration = Duration::from_secs(15 * 60);
#[derive(Clone)]
pub struct AppState {
    pub backend: Backend,
}

pub(crate) struct BareLayout;

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
    let browser_session_lifetime = Duration::from_secs(config.browser_session_lifetime_seconds);
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
        backend: Backend::production(
            Arc::new(config),
            database,
            Arc::new(dataserver),
            Arc::new(oidc),
        ),
    };
    let router = routes()
        .cookies()
        .sessions(
            SessionConfig::builder()
                .lifetime(browser_session_lifetime)
                .build(),
        )
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

pub async fn serve_development() -> anyhow::Result<()> {
    let state = AppState {
        backend: Backend::development(),
    };
    let router = routes()
        .cookies()
        .sessions(SessionConfig::default())
        .app_context(state)
        .build();
    topcoat::start(router)
        .await
        .context("serving development auth portal")
}

pub fn routes() -> RouterBuilder {
    topcoat::router::module_router!()
        // `module_router!` discovers file-based routes, but runtime procedures
        // are registered through the outer router discovery extension.
        .discover()
        .assets(AssetBundle::load().expect("failed to load the Topcoat asset bundle"))
}

pub(crate) fn epoch_seconds() -> anyhow::Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

pub(crate) fn valid_session_token(token: &str) -> bool {
    token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub(crate) async fn current_user(cx: &Cx) -> Result<Option<UserId>> {
    let Some(browser_session) = session::refresh(cx).await? else {
        return Ok(None);
    };
    let expires = browser_session
        .expires_at
        .duration_since(UNIX_EPOCH)?
        .as_secs();
    let app = app_context::<AppState>(cx);
    app.backend
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
    let (mut content, error_page) = match slot {
        Ok(content) => (content, false),
        Err(error) if error.downcast_ref::<RedirectError>().is_some() => return Err(error),
        Err(error) => (friendly_error(cx, &error).await?, true),
    };
    let bare_layout = try_request_context::<BareLayout>(cx).is_some();
    let path = uri(cx).path();
    let home_active = path == "/";
    let groups_active = path == "/groups" || path.starts_with("/groups/");
    let account_active = path == "/account";
    let settings_active = path == "/settings" || path.starts_with("/settings/");
    let nav_class = |active| {
        if active {
            "border-b-2 border-red-800 pb-1 text-sm font-semibold text-red-800 dark:border-red-400 dark:text-red-400"
        } else {
            "border-b-2 border-transparent pb-1 text-sm hover:text-red-800 dark:hover:text-red-400"
        }
    };
    let navigation_user = if bare_layout {
        None
    } else {
        match navigation_user(cx).await {
            Ok(user) => user,
            Err(error) => {
                eprintln!("failed to render Zotero navigation: {error:?}");
                if !error_page {
                    content = friendly_internal_error(cx).await?;
                }
                None
            }
        }
    };
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Zotero"</title>
                <link rel="stylesheet" href=(topcoat::asset::asset!("target/topcoat/tailwind.css"))>
                <link rel="stylesheet" href=(topcoat::asset::asset!("src/app/feedback.css"))>
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body class="min-h-screen bg-white font-sans text-stone-950 dark:bg-stone-950 dark:text-stone-50">
                if !bare_layout {
                    <header class="border-b border-stone-200 dark:border-stone-800">
                        <nav class="mx-auto flex w-full max-w-4xl flex-wrap items-center gap-4 px-4 py-4 sm:px-6" aria-label="Primary navigation">
                            <div class="flex min-w-0 flex-wrap items-center gap-x-5 gap-y-3 sm:gap-x-6">
                                <a class=(if home_active { "border-b-2 border-red-800 pb-1 text-lg font-semibold text-red-800 dark:border-red-400 dark:text-red-400" } else { "border-b-2 border-transparent pb-1 text-lg font-semibold text-red-800 dark:text-red-400" }) href="/" aria-current=(home_active.then_some("page"))>"Zotero"</a>
                                <a class=(nav_class(groups_active)) href="/groups" aria-current=(groups_active.then_some("page"))>"Groups"</a>
                                <a class=(nav_class(account_active)) href="/account" aria-current=(account_active.then_some("page"))>"Account"</a>
                                <a class=(nav_class(settings_active)) href="/settings" aria-current=(settings_active.then_some("page"))>"Settings"</a>
                            </div>
                            <div class="flex w-full min-w-0 items-center justify-between gap-3 sm:ml-auto sm:w-auto">
                                if let Some(user) = &navigation_user {
                                    <span class="min-w-0 truncate text-sm text-stone-600 dark:text-stone-400">
                                        "Signed in as " (user.display_name())
                                    </span>
                                    <form method="post" action="/user/logout">
                                        <button class="border border-stone-300 px-3 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" type="submit">
                                            "Log out"
                                        </button>
                                    </form>
                                } else {
                                    <a class="border border-stone-300 px-3 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" href="/user/login">
                                        "Sign in"
                                    </a>
                                }
                            </div>
                        </nav>
                    </header>
                }
                (content)
            </body>
        </html>
    }
}

async fn navigation_user(cx: &Cx) -> Result<Option<store::UserSummary>> {
    let Some(user_id) = current_user(cx).await? else {
        return Ok(None);
    };
    let app = app_context::<AppState>(cx);
    app.backend.user_by_id(user_id).await.map_err(Into::into)
}

async fn friendly_error(cx: &Cx, error: &topcoat::Error) -> Result<topcoat::view::View> {
    if let Some(error) = error.downcast_ref::<BadRequestError>() {
        friendly_error_page(
            cx,
            StatusCode::BAD_REQUEST,
            "That request could not be completed",
            error.description(),
        )
        .await
    } else if error.downcast_ref::<UnauthorizedError>().is_some() {
        friendly_error_page(
            cx,
            StatusCode::UNAUTHORIZED,
            "Sign in required",
            "Sign in and try that request again.",
        )
        .await
    } else if error.downcast_ref::<ForbiddenError>().is_some() {
        friendly_error_page(
            cx,
            StatusCode::FORBIDDEN,
            "You do not have access",
            "Your account is not allowed to perform that action.",
        )
        .await
    } else if error.downcast_ref::<NotFoundError>().is_some() {
        friendly_error_page(
            cx,
            StatusCode::NOT_FOUND,
            "Page not found",
            "The page may have moved, or you may not have access to it.",
        )
        .await
    } else if error.downcast_ref::<MethodNotAllowedError>().is_some() {
        friendly_error_page(
            cx,
            StatusCode::METHOD_NOT_ALLOWED,
            "Method not allowed",
            "That action is not supported at this address.",
        )
        .await
    } else {
        if error.downcast_ref::<InternalServerError>().is_none() {
            eprintln!("Zotero portal request failed: {error:?}");
        }
        friendly_internal_error(cx).await
    }
}

async fn friendly_internal_error(cx: &Cx) -> Result<topcoat::view::View> {
    friendly_error_page(
        cx,
        StatusCode::INTERNAL_SERVER_ERROR,
        "Something went wrong",
        "The request could not be completed. Please try again.",
    )
    .await
}

async fn friendly_error_page(
    cx: &Cx,
    status: StatusCode,
    title: &str,
    message: &str,
) -> Result<topcoat::view::View> {
    view! { cx =>
        (status)
        <main class="mx-auto w-full max-w-4xl px-4 py-12 sm:px-6 sm:py-16">
            <p class="text-sm font-semibold text-red-800 dark:text-red-400">(status.as_u16())</p>
            <h1 class="mt-2 text-2xl font-semibold">(title)</h1>
            <p class="mt-4 max-w-2xl leading-7 text-stone-600 dark:text-stone-400">(message)</p>
            <a class="mt-6 inline-block border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" href="/">
                "Return to Zotero"
            </a>
        </main>
    }
}

#[page("/{*path}")]
async fn unmatched_page() -> Result {
    Err(not_found().into())
}

pub(crate) async fn signed_in_user(cx: &Cx) -> Result<store::UserSummary> {
    let user_id = authenticated_user(cx).await?;
    let app = app_context::<AppState>(cx);
    app.backend
        .user_by_id(user_id)
        .await?
        .ok_or_else(|| not_found().into())
}

#[component]
pub(crate) async fn username_label(username: &str) -> Result {
    view! {
        <span class="font-mono font-semibold">(username)</span>
    }
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
        <dialog class="fixed inset-0 m-auto max-h-[calc(100vh_-_2rem)] w-[calc(100%_-_2rem)] max-w-lg overflow-y-auto border border-stone-300 bg-white p-0 text-stone-950 shadow-xl backdrop:bg-stone-950/60 dark:border-stone-700 dark:bg-stone-950 dark:text-stone-50" data-confirm-dialog="">
            <div class="p-6">
                <h2 class="text-xl font-semibold" data-confirm-title="">"Confirm action"</h2>
                <p class="mt-3 leading-7 text-stone-600 dark:text-stone-400" data-confirm-message=""></p>
                <div class="mt-5" data-confirm-input-container="" hidden="hidden">
                    <label class="block font-medium" data-confirm-input-label="" for="portal-confirmation"></label>
                    <input class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950" id="portal-confirmation" autocomplete="off" data-confirm-input="">
                    <p class="mt-2 text-sm text-stone-600 dark:text-stone-400" data-confirm-input-help=""></p>
                </div>
                <div class="mt-6 flex flex-wrap justify-end gap-3">
                    <button class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-stone-700 dark:hover:bg-stone-800" type="button" data-confirm-submit="">"Confirm"</button>
                    <button class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700" type="button" data-confirm-cancel="">"Cancel"</button>
                </div>
            </div>
        </dialog>
        <script type="module" src=(topcoat::asset::asset!("src/app/confirmation.js"))></script>
    }
}

#[page]
async fn index(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;

    view! { portal_index(user: &user) }
}

#[topcoat::view::component]
async fn portal_index(user: &store::UserSummary) -> Result {
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <section class="max-w-2xl py-8">
                <h1 class="text-3xl font-semibold">"Welcome, " (user.display_name())</h1>
                <p class="mt-4 text-lg leading-8 text-stone-600 dark:text-stone-400">
                    "This portal manages the server-side parts of your Zotero account, including groups and service settings. Continue using Zotero desktop for library items, attachments, and sync."
                </p>
            </section>
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
        for script in [confirmation, group_actions] {
            assert!(script.contains("new URLSearchParams(new FormData(form))"));
        }
        assert!(group_actions.contains("[data-members-refresh]"));
        assert!(group_actions.contains("X-Zotero-Outcome"));
        assert!(group_actions.contains("already-member"));
        assert!(!group_actions.contains("createContextualFragment"));
    }
}
