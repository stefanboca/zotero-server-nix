mod apps;
mod sessions;
mod storage;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{page, query_params},
    session,
    view::{component, view},
};

use crate::{
    app::{AppState, epoch_seconds, is_topcoat_dev, signed_in_user},
    dataserver::{ConnectedClient, StorageUsage},
    store::{self, BrowserSession},
};

#[topcoat::router::query_params(error = topcoat::router::error::bad_request)]
struct SettingsQuery {
    notice: Option<String>,
    name: Option<String>,
}

#[page]
async fn settings(cx: &Cx) -> Result {
    let parameters = query_params::<SettingsQuery>(cx)?;
    let user = signed_in_user(cx).await?;
    let now = epoch_seconds()?;
    let (usage, browser_sessions, current_handle, connected_clients) = if is_topcoat_dev() {
        (
            StorageUsage {
                total_mb: 1280.0,
                library_mb: 1024.0,
            },
            vec![
                BrowserSession {
                    handle: "current".to_owned(),
                    created_at: now.saturating_sub(7 * 24 * 60 * 60),
                    last_seen_at: now,
                    expires_at: now + 30 * 24 * 60 * 60,
                },
                BrowserSession {
                    handle: "other".to_owned(),
                    created_at: now.saturating_sub(14 * 24 * 60 * 60),
                    last_seen_at: now.saturating_sub(2 * 60 * 60),
                    expires_at: now + 29 * 24 * 60 * 60,
                },
            ],
            "current".to_owned(),
            vec![ConnectedClient {
                handle: "desktop".to_owned(),
                name: "Zotero Desktop".to_owned(),
                date_added: "2026-08-01T10:00:00Z".to_owned(),
                last_used: Some("2026-08-08T09:30:00Z".to_owned()),
            }],
        )
    } else {
        let app = app_context::<AppState>(cx);
        let current_handle = session::token_hash(cx)
            .await?
            .map(|hash| store::session_handle(&hash))
            .unwrap_or_default();
        let usage = app.dataserver.storage_usage(user.user_id).await?;
        let browser_sessions = store::browser_sessions_for_user(&app.database, user.user_id).await?;
        let connected_clients = app.dataserver.connected_clients(user.user_id).await?;
        (usage, browser_sessions, current_handle, connected_clients)
    };
    view! { settings_page(
        usage: usage,
        browser_sessions: &browser_sessions,
        current_handle: &current_handle,
        connected_clients: &connected_clients,
        now: now,
        notice: parameters.notice.as_deref(),
        notice_name: parameters.name.as_deref(),
    ) }
}

#[component]
async fn settings_page(
    usage: StorageUsage,
    browser_sessions: &[BrowserSession],
    current_handle: &str,
    connected_clients: &[ConnectedClient],
    now: u64,
    notice: Option<&str>,
    notice_name: Option<&str>,
) -> Result {
    let toast = match notice {
        Some("browser-session-revoked") => Some("Browser session signed out.".to_owned()),
        Some("client-revoked") => Some(format!(
            "Disconnected {}.",
            notice_name.unwrap_or("the Zotero client")
        )),
        _ => None,
    };
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96" aria-live="polite" aria-atomic="true">
                if let Some(message) = toast.as_deref() {
                    <div class="portal-toast border px-4 py-3 shadow-lg" role="status" data-toast="" data-toast-kind="success">(message)</div>
                }
            </div>
            <h1 class="text-2xl font-semibold">"Settings"</h1>
            <section class="border-b border-stone-200 py-8 dark:border-stone-800" id="storage">
                <h2 class="text-xl font-semibold">"File storage"</h2>
                <dl class="mt-5 grid gap-x-8 gap-y-4 sm:grid-cols-[10rem_1fr]">
                    <dt class="text-stone-600 dark:text-stone-400">"Quota"</dt>
                    <dd class="font-medium">"Unlimited"</dd>
                    <dt class="text-stone-600 dark:text-stone-400">"Used"</dt>
                    <dd class="font-medium">(format_storage_size(usage.total_mb))</dd>
                </dl>
                if usage.total_mb > usage.library_mb {
                    <p class="mt-4 text-sm text-stone-600 dark:text-stone-400">
                        "Usage includes your personal library and group libraries you own."
                    </p>
                }
            </section>

            <section class="border-b border-stone-200 py-8 dark:border-stone-800" id="sessions">
                <h2 class="text-xl font-semibold">"Browser sessions"</h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">"Browsers currently signed in to this portal."</p>
                <ul class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800">
                    for browser_session in browser_sessions {
                        let is_current = browser_session.handle == current_handle;
                        let query = url::form_urlencoded::Serializer::new(String::new())
                            .append_pair("handle", &browser_session.handle)
                            .finish();
                        <li class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between">
                            <div class="min-w-0">
                                <p class="font-medium">
                                    "Browser session"
                                    if is_current {
                                        <span class="ml-2 border border-stone-300 px-2 py-0.5 text-xs dark:border-stone-700">"Current"</span>
                                    }
                                </p>
                                <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                                    "Last active " (format_relative_time(now, browser_session.last_seen_at))
                                    " · Created " (format_relative_time(now, browser_session.created_at))
                                </p>
                            </div>
                            if !is_current {
                                <a class="self-start border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900 sm:self-auto" href=(format!("/settings/sessions?{query}"))>"Sign out"</a>
                            }
                        </li>
                    }
                </ul>
            </section>

            <section class="py-8" id="apps">
                <h2 class="text-xl font-semibold">"Connected apps"</h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">"Zotero clients and other apps authorized to access your libraries."</p>
                if connected_clients.is_empty() {
                    <p class="mt-5 text-stone-600 dark:text-stone-400">"No connected apps."</p>
                } else {
                    <ul class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800">
                        for client in connected_clients {
                            let query = url::form_urlencoded::Serializer::new(String::new())
                                .append_pair("handle", &client.handle)
                                .finish();
                            <li class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between">
                                <div class="min-w-0">
                                    <p class="break-words font-medium">(client.name.as_str())</p>
                                    <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                                        "Last connected " (format_client_date(client.last_used.as_deref()))
                                        " · Added " (format_client_date(Some(&client.date_added)))
                                    </p>
                                </div>
                                <a class="self-start border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900 sm:self-auto" href=(format!("/settings/apps?{query}"))>"Disconnect"</a>
                            </li>
                        }
                    </ul>
                }
            </section>
            <script type="module" src=(topcoat::asset::asset!("src/app/toast.js"))></script>
        </main>
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
    let seconds = now.saturating_sub(timestamp);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} minutes ago", seconds / 60),
        3600..86400 => format!("{} hours ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86400),
    }
}

fn format_client_date(value: Option<&str>) -> String {
    value
        .map(|value| value.trim_end_matches('Z').replace('T', " ") + " UTC")
        .unwrap_or_else(|| "never".to_owned())
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
    }
}
