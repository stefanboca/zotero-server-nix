use jiff::Timestamp;
use topcoat::context::{Cx, app_context};
use topcoat::router::{href, page};
use topcoat::view::{View, component, view};
use topcoat::{Result, session};

use super::{format_relative_time, format_timestamp, parse_client_timestamp};
use crate::app::{confirm_button, section, signed_in_user};
use crate::database::{self, Database};
use crate::dataserver::Dataserver;
use crate::id::UserId;

#[page]
pub(super) async fn security(cx: &Cx) -> Result<impl View> {
    let user_id = signed_in_user(cx).await?.user_id;
    Ok(view! {
        <div class="space-y-6">
            section(browser_sessions(user_id: user_id))
            section(applications(user_id: user_id))
        </div>
    })
}

#[component]
async fn browser_sessions(cx: &Cx, user_id: UserId) -> Result<impl View> {
    let database = app_context::<Database>(cx);
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| database::session_handle(&*hash))
        .unwrap_or_default();
    let mut browser_sessions = database
        .browser_sessions_for_user(app_context::<Dataserver>(cx), user_id)
        .await?;
    browser_sessions.sort_by_key(|browser_session| browser_session.handle != current_handle);
    let now = Timestamp::now();
    Ok(view! {
        <div id="sessions">
            <h3 class="portal-section-heading">"Browser Sessions"</h3>
            <p class="mt-2">
                "These are active sessions that are logged in to this account in a browser on this Zotero server."
            </p>
            <div class="portal-table-scroll">
                <table class="portal-table min-w-[40rem]">
                    <thead>
                        <tr>
                            <th scope="col">"Platform"</th>
                            <th scope="col">"Last Used"</th>
                            <th scope="col">"Expires"</th>
                            <th scope="col">"Delete Session"</th>
                        </tr>
                    </thead>
                    <tbody>
                        for browser_session in browser_sessions {
                            let is_current = browser_session.handle == current_handle;
                            let last_active = browser_session.last_seen_at;
                            let expires = browser_session.expires_at;
                            let expires_text = format_timestamp(expires);
                            <tr data-browser-session-item="">
                                <td class="font-medium">
                                    (browser_platform(
                                        browser_session.user_agent.as_deref(),
                                    ))
                                    if is_current {
                                        <span class="ml-2 text-xs font-normal">"Current"</span>
                                    }
                                </td>
                                <td>
                                    if is_current {
                                        "Now"
                                    } else {
                                        <time datetime=(last_active.to_string())>
                                            (format_relative_time(now, last_active))
                                        </time>
                                    }
                                </td>
                                <td>
                                    <time datetime=(expires.to_string())>
                                        (expires_text.as_str())
                                    </time>
                                </td>
                                <td>
                                    <form
                                        method="post"
                                        action=(href!(super::sessions::revoke_session))
                                    >
                                        <input
                                            type="hidden"
                                            name="handle"
                                            value=(browser_session.handle.as_str())
                                        >
                                        confirm_button(
                                            key: browser_session.handle.as_str(),
                                            label: if is_current {
                                                "Log Out"
                                            } else {
                                                "Remove Session"
                                            },
                                            confirmation: if is_current {
                                                "Log out this browser session?"
                                            } else {
                                                "Remove this browser session?"
                                            },
                                            class: "portal-link-danger whitespace-nowrap"
                                        )
                                    </form>
                                </td>
                            </tr>
                        }
                    </tbody>
                </table>
            </div>
        </div>
    })
}

fn browser_platform(user_agent: Option<&str>) -> String {
    let Some(user_agent) = user_agent else {
        return "Unknown".to_owned();
    };
    let browser = if user_agent.contains("Edg/") {
        Some("Edge")
    } else if user_agent.contains("OPR/") {
        Some("Opera")
    } else if user_agent.contains("FxiOS/") || user_agent.contains("Firefox/") {
        Some("Firefox")
    } else if user_agent.contains("CriOS/") || user_agent.contains("Chrome/") {
        Some("Chrome")
    } else if user_agent.contains("Version/") && user_agent.contains("Safari/") {
        Some("Safari")
    } else {
        None
    };
    let platform = if user_agent.contains("Windows NT") {
        Some("Windows")
    } else if user_agent.contains("Android") {
        Some("Android")
    } else if user_agent.contains("iPhone") || user_agent.contains("iPad") {
        Some("iOS")
    } else if user_agent.contains("Macintosh") || user_agent.contains("Mac OS X") {
        Some("macOS")
    } else if user_agent.contains("CrOS") {
        Some("ChromeOS")
    } else if user_agent.contains("Linux") {
        Some("Linux")
    } else {
        None
    };
    match (browser, platform) {
        (Some(browser), Some(platform)) => format!("{browser} on {platform}"),
        (Some(browser), None) => browser.to_owned(),
        (None, Some(platform)) => format!("Browser on {platform}"),
        (None, None) => "Unknown".to_owned(),
    }
}

#[component]
async fn applications(cx: &Cx, user_id: UserId) -> Result<impl View> {
    let access_keys = app_context::<Dataserver>(cx).access_keys(user_id).await?;
    let now = Timestamp::now();
    Ok(view! {
        <div id="applications">
            <div class="flex flex-wrap items-start justify-between gap-4">
                <div>
                    <h2 class="portal-section-heading">"Applications"</h2>
                    <p class="mt-2">
                        "These are keys used by official Zotero applications or third party applications to access your synced data."
                    </p>
                </div>
                <div class="flex shrink-0 flex-col items-start gap-2">
                    <a
                        class="portal-button portal-button-secondary"
                        href=(href!(super::keys::new_key_route))
                    >
                        "Create new private key"
                    </a>
                    <p>
                        <strong>"User ID:"</strong>
                        " Your user ID for use in API calls is "
                        <span class="font-mono">(user_id.to_string())</span>
                        "."
                    </p>
                </div>
            </div>
            if access_keys.is_empty() {
                <p class="mt-5">"No application keys."</p>
            } else {
                <div class="portal-table-scroll">
                    <table class="portal-table">
                        <thead>
                            <tr>
                                <th scope="col">"Name"</th>
                                <th scope="col">"Last Used"</th>
                                <th scope="col">"Last Used IPs"</th>
                                <th scope="col">"Edit Key"</th>
                                <th scope="col">"Delete Key"</th>
                            </tr>
                        </thead>
                        <tbody>
                            for key in access_keys {
                                let last_connected = parse_client_timestamp(
                                    key.last_used.as_deref(),
                                )?;
                                let recent_ips = if key.recent_ips.is_empty() {
                                    "--".to_owned()
                                } else {
                                    key.recent_ips.join(", ")
                                };
                                <tr data-key-item="">
                                    <td class="break-words font-medium">
                                        (key.name.as_str())
                                    </td>
                                    <td>
                                        if let Some(last_connected) = last_connected {
                                            <time datetime=(last_connected.to_string())>
                                                (format_relative_time(now, last_connected))
                                            </time>
                                        } else {
                                            "--"
                                        }
                                    </td>
                                    <td class="break-all">(recent_ips)</td>
                                    <td>
                                        let key_id = key
                                            .key
                                            .parse::<super::keys::ApiKeyValue>()
                                            .map_err(anyhow::Error::msg)?;
                                        <a
                                            class="portal-link"
                                            href=(href!(
                                        super::keys::edit_key_route,
                                        super::keys::EditKeyId(key_id),
                                    ))
                                        >
                                            "Edit Key"
                                        </a>
                                    </td>
                                    <td>
                                        <form
                                            method="post"
                                            action=(href!(super::keys::revoke_key))
                                        >
                                            <input type="hidden" name="key" value=(key.key.as_str())>
                                            let delete_confirmation = format!(
                                        "Delete API key “{}”?",
                                        key.name,
                                    );
                                            confirm_button(
                                                key: key.key.as_str(),
                                                label: "Delete Key",
                                                confirmation: delete_confirmation.as_str(),
                                                class: "portal-link-danger whitespace-nowrap"
                                            )
                                        </form>
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
            }
        </div>
    })
}
