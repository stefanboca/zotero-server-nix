mod _web_library;
mod auth;
mod export;
mod groups;
mod import;
mod mylibrary;
mod profile;
mod search;
mod settings;
mod translation;
mod user;
mod web;

use jiff::SignedDuration;
use topcoat::context::{Cx, app_context, memoize, try_request_context};
use topcoat::router::error::{RedirectError, RewriteError, not_found, redirect};
use topcoat::router::request::{headers, uri};
use topcoat::router::response::IntoResponse;
use topcoat::router::tower::TowerLayer;
use topcoat::router::{RouterBuilder, RouterBuilderDiscoverExt, Slot, StatusCode, href, layout};
use topcoat::runtime::RouterBuilderRuntimeExt as _;
use topcoat::view::{Child, PromotedStr, StaticStr, View, component, error_boundary, view};
use topcoat::{Result, session};
use tower_http::LatencyUnit;
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::app::_web_library::WebLibrary;
use crate::database::{self, Database};
use crate::dataserver::Dataserver;
use crate::id::UserId;

pub(crate) const FLOW_LIFETIME: SignedDuration = SignedDuration::from_mins(15);

pub fn routes() -> RouterBuilder {
    topcoat::router::module_router!()
        .runtime()
        // Shards and explicit-path compatibility routes are registered through
        // the outer discovery extension.
        .discover()
        .layer(TowerLayer::new(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Millis),
                )
                .on_failure(
                    DefaultOnFailure::new()
                        .level(Level::ERROR)
                        .latency_unit(LatencyUnit::Millis),
                ),
        ))
}

#[memoize]
async fn memoized_current_user(cx: &Cx) -> Result<Option<UserId>> {
    let Some(browser_session) = session::refresh(cx).await? else {
        return Ok(None);
    };
    let expires = browser_session.expires_at.try_into()?;
    let user_agent = headers(cx)
        .get("user-agent")
        .and_then(|value| value.to_str().ok());
    app_context::<Database>(cx)
        .refresh_browser_session(
            app_context::<Dataserver>(cx),
            &*browser_session.token_hash,
            expires,
            user_agent,
        )
        .await
        .map_err(Into::into)
}

pub(crate) async fn current_user(cx: &Cx) -> Result<Option<UserId>> {
    match memoized_current_user(cx).await {
        Ok(user_id) => Ok(*user_id),
        Err(error) => {
            tracing::error!(error = %format_args!("{error:#}"), "failed to authenticate request");
            Err(anyhow::anyhow!("failed to authenticate request").into())
        }
    }
}

pub(crate) async fn authenticated_user(cx: &Cx) -> Result<UserId> {
    if let Some(user_id) = current_user(cx).await? {
        return Ok(user_id);
    }

    let return_path =
        normalized_return_path(uri(cx).path_and_query().map_or("/", |path| path.as_str()))
            .unwrap_or_else(|| "/".to_owned());
    Err(redirect(
        href!(user::login::login)
            .query(user::login::LoginQuery {
                session: None,
                redirect_to: Some(return_path),
            })
            .resolve(cx),
    )
    .into())
}

pub(crate) fn normalized_return_path(value: &str) -> Option<String> {
    if value.len() > 2048
        || !value.starts_with('/')
        || value.starts_with("//")
        || value.contains('\\')
    {
        return None;
    }

    let base = url::Url::parse("https://zotero.invalid").unwrap();
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

#[layout]
async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let web_library_active = try_request_context::<WebLibrary>(cx).is_some();
    let show_portal_navigation = !web_library_active;
    let navigation_user = if show_portal_navigation {
        match current_user(cx).await? {
            Some(user_id) => authenticated_user_summary(cx, user_id).await?,
            None => None,
        }
    } else {
        None
    };
    let landing = href!(_web_library::landing);
    let groups = href!(groups::list_groups);
    Ok(view! {
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
                if web_library_active {
                    <link
                        rel="stylesheet"
                        href="/static/web-library/zotero-web-library.css"
                    >
                } else {
                    <link
                        rel="stylesheet"
                        href=(topcoat::asset::asset!("src/app/portal.css"))
                    >
                }
                topcoat::runtime::script()
                topcoat::dev::script()
            </head>
            <body
                class=(if web_library_active {
                    PromotedStr(&"min-h-screen")
                } else {
                    PromotedStr(&"portal-theme")
                })
            >
                if show_portal_navigation {
                    <header class="portal-header border-b">
                        <nav
                            class="portal-navbar mx-auto flex w-full flex-wrap items-center gap-4"
                            aria-label="Primary navigation"
                        >
                            <div class="flex min-w-0 items-center">
                                <a
                                    class="portal-brand"
                                    href=(landing.resolve(cx))
                                    aria-current=(landing
                                        .is_current(cx)
                                        .then_some(PromotedStr(&"page")))
                                >
                                    <img
                                        class="portal-logo"
                                        src=(topcoat::asset::asset!("src/app/zotero-logo.svg"))
                                        width="108"
                                        height="32"
                                        alt="Zotero"
                                    >
                                </a>
                            </div>
                            <div
                                class="flex w-full min-w-0 items-center justify-end sm:ml-auto sm:w-auto"
                            >
                                <a
                                    class="portal-nav-link"
                                    href=(groups.resolve(cx))
                                    aria-current=(groups
                                        .is_current(cx)
                                        .then_some(PromotedStr(&"page")))
                                >
                                    "Groups"
                                </a>
                                if let Some(user) = &navigation_user {
                                    <details class="relative">
                                        <summary class="portal-user-menu cursor-pointer list-none">
                                            (user.username.as_str())
                                        </summary>
                                        <div
                                            class="portal-dropdown absolute right-0 z-20 mt-2 w-48 rounded-md border p-2 shadow-lg"
                                        >
                                            <a
                                                class="portal-dropdown-link block rounded px-3 py-2 text-sm"
                                                href=(href!(
                                                    profile::profile,
                                                    profile::ProfileUsername(user.username.clone()),
                                                ))
                                            >
                                                "My Profile"
                                            </a>
                                            <div class="portal-dropdown-divider" role="separator"></div>
                                            <a
                                                class="portal-dropdown-link block rounded px-3 py-2 text-sm"
                                                href=(href!(settings::settings))
                                            >
                                                "Settings"
                                            </a>
                                            <form method="post" action=(href!(user::logout_route))>
                                                <button
                                                    class="portal-dropdown-link block w-full rounded px-3 py-2 text-left text-sm"
                                                    type="submit"
                                                >
                                                    "Log Out"
                                                </button>
                                            </form>
                                        </div>
                                    </details>
                                }
                            </div>
                        </nav>
                    </header>
                }
                error_boundary(fallback: |error| friendly_error_page(cx, error), (slot))
            </body>
        </html>
    })
}

#[component]
pub(crate) async fn confirm_button(
    label: &str,
    confirmation: &str,
    class: &str,
) -> Result<impl View> {
    Ok(view! {
        <button
            class=(class)
            type="submit"
            data-confirm=(confirmation)
            @click="return window.confirm(this.dataset.confirm)"
        >
            (label)
        </button>
    })
}

/// Responsive content boundary shared by portal pages. The Web Library does
/// not use it because that application owns the entire viewport.
#[component]
pub(crate) async fn portal_page(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <main class="portal-page">(child)</main> })
}

pub(crate) struct SidebarItem {
    pub(crate) href: String,
    pub(crate) label: PromotedStr,
    pub(crate) active: bool,
}

#[component]
pub(crate) async fn sidebar(aria_label: PromotedStr, items: Vec<SidebarItem>) -> Result<impl View> {
    Ok(view! {
        <nav class="portal-sidebar" aria-label=(aria_label)>
            <ul class="portal-sidebar-items">
                for item in items {
                    <li>
                        <a
                            class=(if item.active {
                                "portal-sidebar-link portal-sidebar-link-active"
                            } else {
                                "portal-sidebar-link"
                            })
                            href=(item.href)
                            aria-current=(item.active.then_some("page"))
                        >
                            (item.label)
                        </a>
                    </li>
                }
            </ul>
        </nav>
    })
}

/// The modern zotero.org settings theme uses the same bordered white surface
/// for independent sections. Keeping it here prevents pages from drifting.
#[component]
pub(crate) async fn section(child: Child<'_>) -> Result<impl View> {
    Ok(view! { <section class="portal-card">(child)</section> })
}

#[component]
pub(crate) async fn success_message(message: &str) -> Result<impl View> {
    Ok(view! {
        <p class="portal-alert portal-alert-success mb-6" role="status">(message)</p>
    })
}

fn friendly_error_page(cx: &Cx, error: topcoat::Error) -> Result<impl View> {
    if error.downcast_ref::<RedirectError>().is_some()
        || error.downcast_ref::<RewriteError>().is_some()
    {
        return Err(error);
    }
    let status = error
        .into_response(cx)
        .expect("error.into_response() always return Ok")
        .status();
    let message = match status {
        StatusCode::UNAUTHORIZED => "Sign in and try again",
        StatusCode::FORBIDDEN => "Your account is not allowed to perform that action.",
        StatusCode::NOT_FOUND => "The page could not be found. Check the address and try again.",
        _ => "Something went wrong. Please try again",
    };
    Ok(view! {
        cx =>
        (status)
        <main class="mx-auto w-full max-w-4xl px-4 py-12 sm:px-6 sm:py-16">
            <h1 class="text-4xl font-semibold text-red-800">
                (status.as_u16())
                if let Some(title) = status.canonical_reason() {
                    " — "
                    (StaticStr(title))
                }
            </h1>
            <p class="mt-4 max-w-2xl leading-7 text-stone-600">(StaticStr(message))</p>
        </main>
    })
}

pub(crate) async fn signed_in_user(cx: &Cx) -> Result<database::UserSummary> {
    let user_id = authenticated_user(cx).await?;
    authenticated_user_summary(cx, user_id)
        .await?
        .ok_or_else(|| not_found().into())
}

#[memoize]
async fn memoized_authenticated_user_summary(
    cx: &Cx,
    user_id: UserId,
) -> Result<Option<database::UserSummary>> {
    app_context::<Database>(cx)
        .user_by_id(user_id)
        .await
        .map_err(Into::into)
}

async fn authenticated_user_summary(
    cx: &Cx,
    user_id: UserId,
) -> Result<Option<database::UserSummary>> {
    match memoized_authenticated_user_summary(cx, user_id).await {
        Ok(user) => Ok(user.clone()),
        Err(error) => {
            tracing::error!(
                user_id = %user_id,
                error = %format_args!("{error:#}"),
                "failed to look up authenticated user",
            );
            Err(anyhow::anyhow!("failed to authenticate request").into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_build() {
        let _ = routes().build();
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
    fn browser_mutations_use_standard_routes() {
        let group_actions = include_str!("app/groups/group_id/group_actions.js");
        assert!(group_actions.contains("method: \"PATCH\""));
        assert!(group_actions.contains("Content-Type\": \"application/json"));
        assert!(group_actions.contains("[data-member-roles]"));
        assert!(!group_actions.contains("window.confirm"));
        assert!(!group_actions.contains("/_topcoat/procedures/"));
        assert!(!group_actions.contains("createContextualFragment"));
    }

    #[test]
    fn newly_created_keys_replace_the_editor_with_the_one_time_secret() {
        let editor = include_str!("app/settings/keys/key_editor.js");
        assert!(editor.contains("[data-key-editor-fields]"));
        assert!(editor.contains(".hidden = true"));
        assert!(editor.contains("created.hidden = false"));
    }
}
