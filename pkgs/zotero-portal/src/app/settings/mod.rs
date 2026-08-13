mod apps;
mod sessions;
mod storage;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, query_params},
    runtime::shard,
    session,
    view::{component, view},
};

use crate::{
    app::{confirmation_dialog, signed_in_user, toast_script, username_label},
    backend::{self, Backend, UserSummary},
    dataserver::StorageUsage,
    epoch_seconds,
};

#[topcoat::router::query_params(error = bad_request)]
struct SettingsQuery {
    notice: Option<String>,
    name: Option<String>,
}

#[page]
async fn settings(cx: &Cx) -> Result {
    let parameters = query_params::<SettingsQuery>(cx)?;
    let user = signed_in_user(cx).await?;
    let backend = app_context::<Backend>(cx);
    let usage = backend.storage_usage(user.user_id).await?;
    let toast = match parameters.notice.as_deref() {
        Some("browser-session-revoked") => Some("Browser session signed out.".to_owned()),
        Some("client-revoked") => Some(format!(
            "Disconnected {}.",
            parameters.name.as_deref().unwrap_or("the Zotero client")
        )),
        Some("clients-revoked") => Some("Disconnected all apps.".to_owned()),
        _ => None,
    };
    view! {
        signal browser_sessions_refresh = false;
        signal connected_apps_refresh = false;

        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div
                class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96"
                aria-live="polite"
                aria-atomic="true"
                data-toast-region=""
            >
                if let Some(message) = toast.as_deref() {
                    <div
                        class="portal-toast border px-4 py-3 shadow-lg"
                        role="status"
                        data-toast=""
                        data-toast-kind="success"
                    >
                        (message)
                    </div>
                }
            </div>
            <h1 class="text-2xl font-semibold">"Settings"</h1>
            <section
                id="account"
                class="py-8 border-b border-stone-200 dark:border-stone-800"
            >
                account_section(user: &user)
            </section>
            <section
                id="storage"
                class="py-8 border-b border-stone-200 dark:border-stone-800"
            >
                storage_section(usage: &usage)
            </section>

            <section
                id="sessions"
                class="py-8 border-b border-stone-200 dark:border-stone-800"
            >
                <h2 class="text-xl font-semibold">"Browser sessions"</h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
                    "Browsers currently signed in to this portal."
                </p>
                <input
                    type="checkbox"
                    hidden="hidden"
                    data-browser-sessions-refresh=""
                    @change=$(|event: topcoat::runtime::Event| {
                        browser_sessions_refresh.set(event.target.checked)
                    })
                >
                browser_sessions_shard(refresh: $(browser_sessions_refresh.get()))
            </section>

            <section id="apps" class="py-8">
                <h2 class="text-xl font-semibold">"Connected apps"</h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
                    "Zotero clients and other apps authorized to access your libraries."
                </p>
                <input
                    type="checkbox"
                    hidden="hidden"
                    data-connected-apps-refresh=""
                    @change=$(|event: topcoat::runtime::Event| {
                        connected_apps_refresh.set(event.target.checked)
                    })
                >
                connected_apps_shard(refresh: $(connected_apps_refresh.get()))
            </section>
            confirmation_dialog()
            toast_script()
        </main>
    }
}

#[component]
async fn account_section(user: &UserSummary) -> Result {
    view! {
        <h2 class="text-xl font-semibold">"Account"</h2>
        <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
            <dt class="text-stone-600 dark:text-stone-400">"Name"</dt>
            <dd class="font-medium">(user.display_name())</dd>
            <dt class="text-stone-600 dark:text-stone-400">"Username"</dt>
            <dd class="break-all">username_label(username: user.username.as_str())</dd>
            <dt class="text-stone-600 dark:text-stone-400">"Email"</dt>
            <dd class="font-medium break-all">(user.email.as_str())</dd>
        </dl>
        <p class="mt-4 text-sm text-stone-600 dark:text-stone-400">
            "Your identity is managed by the OpenID Connect provider."
        </p>
    }
}

#[component]
async fn storage_section(usage: &StorageUsage) -> Result {
    view! {
        <h2 class="text-xl font-semibold">"File storage"</h2>
        <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
            <dt class="text-stone-600 dark:text-stone-400">"Quota"</dt>
            <dd class="font-medium">"Unlimited"</dd>
            <dt class="text-stone-600 dark:text-stone-400">"Used (library)"</dt>
            <dd class="font-medium">(format_storage_size(usage.library_mb))</dd>
            <dt class="text-stone-600 dark:text-stone-400">"Used (total)"</dt>
            <dd class="font-medium">(format_storage_size(usage.total_mb))</dd>
        </dl>
        <p class="mt-4 text-sm text-stone-600 dark:text-stone-400">
            "Usage includes your personal library and group libraries you own."
        </p>
    }
}

#[shard]
async fn browser_sessions_shard(cx: &Cx, refresh: bool) -> Result {
    let _ = refresh;
    let user = signed_in_user(cx).await?;
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| backend::session_handle(&*hash))
        .unwrap_or_default();
    let browser_sessions = app_context::<Backend>(cx)
        .browser_sessions_for_user(user.user_id)
        .await?;
    let now = epoch_seconds()?;
    view! {
        <ul
            class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800"
        >
            for browser_session in browser_sessions {
                let is_current = browser_session.handle == current_handle;
                let last_active = display_epoch_timestamp(
                    now,
                    browser_session.last_seen_at,
                )?;
                let created = display_epoch_timestamp(now, browser_session.created_at)?;
                let expires = display_epoch_timestamp(now, browser_session.expires_at)?;
                <li
                    class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between"
                    data-confirm-item=""
                    data-browser-session-item=""
                >
                    <div class="min-w-0">
                        <p class="font-medium">
                            "Browser session"
                            if is_current {
                                <span
                                    class="ml-2 border border-stone-300 px-2 py-0.5 text-xs dark:border-stone-700"
                                >
                                    "Current"
                                </span>
                            }
                        </p>
                        <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                            "Last active "
                            <time
                                datetime=(last_active.datetime.as_str())
                                title=(last_active.absolute.as_str())
                            >
                                (last_active.relative.as_str())
                            </time>
                            " · Created "
                            <time
                                datetime=(created.datetime.as_str())
                                title=(created.absolute.as_str())
                            >
                                (created.relative.as_str())
                            </time>
                            " · Expires "
                            <time
                                datetime=(expires.datetime.as_str())
                                title=(expires.absolute.as_str())
                            >
                                (expires.relative.as_str())
                            </time>
                        </p>
                    </div>
                    if !is_current {
                        <form
                            method="post"
                            action="/settings/sessions"
                            data-confirm="Sign out this browser session?"
                            data-confirm-action="Sign out"
                            data-confirm-success="Browser session signed out."
                            data-confirm-error="Couldn’t sign out the browser session. Please try again."
                            data-confirm-refresh="[data-browser-sessions-refresh]"
                            data-confirm-stay="true"
                        >
                            <input type="hidden" name="all" value="false">
                            <input
                                type="hidden"
                                name="handle"
                                value=(browser_session.handle.as_str())
                            >
                            <button
                                class="self-start border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900 sm:self-auto"
                                type="submit"
                            >
                                "Sign out"
                            </button>
                        </form>
                    }
                </li>
            }
        </ul>
        <div class="mt-6 flex justify-end">
            <form
                method="post"
                action="/settings/sessions"
                data-confirm="Sign out every browser session, including this one?"
                data-confirm-action="Sign out all sessions"
                data-confirm-success="All browser sessions signed out."
                data-confirm-error="Couldn’t sign out all browser sessions. Please try again."
            >
                <input type="hidden" name="all" value="true">
                <button
                    class="border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                    type="submit"
                >
                    "Sign out all sessions"
                </button>
            </form>
        </div>
    }
}

#[shard]
async fn connected_apps_shard(cx: &Cx, refresh: bool) -> Result {
    let _ = refresh;
    let user = signed_in_user(cx).await?;
    let connected_clients = app_context::<Backend>(cx)
        .connected_clients(user.user_id)
        .await?;
    let now = epoch_seconds()?;
    view! {
        if connected_clients.is_empty() {
            <p class="mt-5 text-stone-600 dark:text-stone-400">"No connected apps."</p>
        } else {
            <ul
                class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800"
            >
                for client in connected_clients {
                    let last_connected = display_client_timestamp(
                        now,
                        client.last_used.as_deref(),
                    )?;
                    let added = display_client_timestamp(
                        now,
                        Some(&client.date_added),
                    )?;
                    <li
                        class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between"
                        data-confirm-item=""
                        data-client-item=""
                    >
                        <div class="min-w-0">
                            <p class="break-words font-medium">
                                (client.name.as_str())
                            </p>
                            <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                                "Last connected "
                                if let Some(last_connected) = last_connected
                                    .as_ref() {
                                    <time
                                        datetime=(last_connected.datetime.as_str())
                                        title=(last_connected.absolute.as_str())
                                    >
                                        (last_connected.relative.as_str())
                                    </time>
                                } else {
                                    "never"
                                }
                                " · Added "
                                if let Some(added) = added.as_ref() {
                                    <time
                                        datetime=(added.datetime.as_str())
                                        title=(added.absolute.as_str())
                                    >
                                        (added.relative.as_str())
                                    </time>
                                } else {
                                    "unknown"
                                }
                            </p>
                        </div>
                        <form
                            method="post"
                            action="/settings/apps"
                            data-confirm=(format!("Disconnect “{}”?", client.name))
                            data-confirm-action="Disconnect"
                            data-confirm-success=(format!("Disconnected {}.", client.name))
                            data-confirm-error=(format!(
                                "Couldn’t disconnect {}. Please try again.", client.name
                            ))
                            data-confirm-refresh="[data-connected-apps-refresh]"
                            data-confirm-stay="true"
                        >
                            <input type="hidden" name="all" value="false">
                            <input
                                type="hidden"
                                name="handle"
                                value=(client.handle.as_str())
                            >
                            <button
                                class="self-start border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900 sm:self-auto"
                                type="submit"
                            >
                                "Disconnect"
                            </button>
                        </form>
                    </li>
                }
            </ul>
            <div class="mt-6 flex justify-end">
                <form
                    method="post"
                    action="/settings/apps"
                    data-confirm="Disconnect every Zotero client and app?"
                    data-confirm-action="Disconnect all apps"
                    data-confirm-success="Disconnected all apps."
                    data-confirm-error="Couldn’t disconnect all apps. Please try again."
                    data-confirm-refresh="[data-connected-apps-refresh]"
                    data-confirm-stay="true"
                >
                    <input type="hidden" name="all" value="true">
                    <button
                        class="border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                        type="submit"
                    >
                        "Disconnect all apps"
                    </button>
                </form>
            </div>
        }
    }
}

fn format_storage_size(mebibytes: f64) -> String {
    const MIB_PER_GIB: f64 = 1024.0;
    const MIB_PER_TIB: f64 = MIB_PER_GIB * 1024.0;
    if mebibytes >= MIB_PER_TIB {
        format!("{:.2} TiB", mebibytes / MIB_PER_TIB)
    } else if mebibytes >= MIB_PER_GIB {
        format!("{:.2} GiB", mebibytes / MIB_PER_GIB)
    } else {
        format!("{mebibytes:.1} MiB")
    }
}

fn format_relative_time(now: u64, timestamp: u64) -> String {
    if timestamp > now {
        return format_future_time(timestamp - now);
    }
    let seconds = now - timestamp;
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} ago", time_unit(seconds / 60, "minute")),
        3600..86400 => format!("{} ago", time_unit(seconds / 3600, "hour")),
        86400..2_592_000 => format!("{} ago", time_unit(seconds / 86400, "day")),
        2_592_000..31_536_000 => {
            format!("{} ago", time_unit(seconds / 2_592_000, "month"))
        }
        _ => format!("{} ago", time_unit(seconds / 31_536_000, "year")),
    }
}

fn format_future_time(seconds: u64) -> String {
    match seconds {
        0..60 => "in less than a minute".to_owned(),
        60..3600 => format!("in {}", time_unit(seconds / 60, "minute")),
        3600..86400 => format!("in {}", time_unit(seconds / 3600, "hour")),
        86400..2_592_000 => format!("in {}", time_unit(seconds / 86400, "day")),
        2_592_000..31_536_000 => {
            format!("in {}", time_unit(seconds / 2_592_000, "month"))
        }
        _ => format!("in {}", time_unit(seconds / 31_536_000, "year")),
    }
}

fn time_unit(value: u64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit}")
    } else {
        format!("{value} {unit}s")
    }
}

struct DisplayTimestamp {
    datetime: String,
    absolute: String,
    relative: String,
}

fn display_epoch_timestamp(now: u64, timestamp: u64) -> Result<DisplayTimestamp> {
    let datetime = OffsetDateTime::from_unix_timestamp(i64::try_from(timestamp)?)?;
    display_timestamp(now, datetime)
}

fn display_client_timestamp(now: u64, value: Option<&str>) -> Result<Option<DisplayTimestamp>> {
    value
        .map(|value| OffsetDateTime::parse(value, &Rfc3339))
        .transpose()?
        .map(|datetime| display_timestamp(now, datetime))
        .transpose()
}

fn display_timestamp(now: u64, datetime: OffsetDateTime) -> Result<DisplayTimestamp> {
    let timestamp = u64::try_from(datetime.unix_timestamp())?;
    let datetime = datetime.format(&Rfc3339)?;
    let absolute = format!("{} UTC", datetime.trim_end_matches('Z').replace('T', " "));
    Ok(DisplayTimestamp {
        datetime,
        absolute,
        relative: format_relative_time(now, timestamp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_dataserver_storage_units() {
        assert_eq!(format_storage_size(0.0), "0.0 MiB");
        assert_eq!(format_storage_size(1536.0), "1.50 GiB");
        assert_eq!(format_storage_size(2.0 * 1024.0 * 1024.0), "2.00 TiB");
    }

    #[test]
    fn formats_session_activity_without_client_javascript() {
        assert_eq!(format_relative_time(10_000, 10_000), "just now");
        assert_eq!(format_relative_time(10_000, 9_880), "2 minutes ago");
        assert_eq!(format_relative_time(10_000, 2_800), "2 hours ago");
        assert_eq!(format_relative_time(10_000, 10_120), "in 2 minutes");
    }

    #[test]
    fn formats_exact_client_time_for_native_tooltips() {
        let timestamp = display_client_timestamp(1_786_176_000, Some("2026-08-08T09:30:00Z"))
            .expect("valid timestamp")
            .expect("present timestamp");
        assert_eq!(timestamp.datetime, "2026-08-08T09:30:00Z");
        assert_eq!(timestamp.absolute, "2026-08-08 09:30:00 UTC");
    }
}
