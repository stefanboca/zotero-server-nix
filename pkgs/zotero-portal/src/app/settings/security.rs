use jiff::Timestamp;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{href, page},
    session,
    view::{component, view},
};

use crate::{
    app::confirm_button,
    database::{self, Database},
    dataserver::Dataserver,
    id::UserId,
};

use super::{display_client_timestamp, display_timestamp, settings_user};

#[page]
pub(super) async fn security(cx: &Cx) -> Result {
    let user_id = settings_user(cx).user_id;
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Security"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Manage browser sessions and API access keys."
            </p>
        </header>
        browser_sessions(user_id: user_id)
        applications(user_id: user_id)
    }
}

#[component]
async fn browser_sessions(cx: &Cx, user_id: UserId) -> Result {
    let database = app_context::<Database>(cx);
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| database::session_handle(&*hash))
        .unwrap_or_default();
    let browser_sessions = database
        .browser_sessions_for_user(app_context::<Dataserver>(cx), user_id)
        .await?;
    let now = Timestamp::now();
    let current_handle = current_handle.as_str();
    view! {
        <section
            id="sessions"
            class="border-b border-stone-200 py-8 dark:border-stone-800"
        >
            <h2 class="text-xl font-semibold">"Browser Sessions"</h2>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Browsers currently signed in to this portal."
            </p>
            <ul
                class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800"
            >
                for browser_session in browser_sessions {
                    let is_current = browser_session.handle == current_handle;
                    let last_active = display_timestamp(now, browser_session.last_seen_at);
                    let created = display_timestamp(now, browser_session.created_at);
                    let expires = display_timestamp(now, browser_session.expires_at);
                    <li
                        class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between"
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
                                class: "self-start border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900 sm:self-auto"
                            )
                        </form>
                    </li>
                }
            </ul>
        </section>
    }
}

#[component]
async fn applications(cx: &Cx, user_id: UserId) -> Result {
    let access_keys = app_context::<Dataserver>(cx).access_keys(user_id).await?;
    let now = Timestamp::now();
    view! {
        <section id="applications" class="py-8">
            <div class="flex flex-wrap items-start justify-between gap-4">
                <div>
                    <h2 class="text-xl font-semibold">"Applications"</h2>
                    <p class="mt-2 text-stone-600 dark:text-stone-400">
                        "API keys authorized to access your libraries."
                    </p>
                    <p class="mt-2 text-sm text-stone-600 dark:text-stone-400">
                        "User ID: Your user ID for API calls is "
                        <strong class="font-mono text-stone-950 dark:text-stone-50">
                            (user_id.to_string())
                        </strong>
                        "."
                    </p>
                </div>
                <a
                    class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                    href=(href!(super::keys::new_key_route))
                >
                    "Create new private key"
                </a>
            </div>
            if access_keys.is_empty() {
                <p class="mt-5 text-stone-600 dark:text-stone-400">
                    "No application keys."
                </p>
            } else {
                <ul
                    class="mt-5 divide-y divide-stone-200 border-y border-stone-200 dark:divide-stone-800 dark:border-stone-800"
                >
                    for key in access_keys {
                        let last_connected = display_client_timestamp(
                            now,
                            key.last_used.as_deref(),
                        )?;
                        let added = display_client_timestamp(
                            now,
                            (!key.date_added.is_empty())
                                .then_some(key.date_added.as_str()),
                        )?;
                        let recent_ips = if key.recent_ips.is_empty() {
                            "None".to_owned()
                        } else {
                            key.recent_ips.join(", ")
                        };
                        <li
                            class="flex flex-col gap-3 py-4 sm:flex-row sm:items-center sm:justify-between"
                            data-key-item=""
                        >
                            <div class="min-w-0">
                                <p class="break-words font-medium">(key.name.as_str())</p>
                                <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                                    "Last connected "
                                    if let Some(last_connected) = last_connected.as_ref() {
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
                                <p
                                    class="mt-1 break-all text-sm text-stone-600 dark:text-stone-400"
                                >
                                    "Recent IPs: "
                                    (recent_ips)
                                </p>
                            </div>
                            <div class="flex flex-wrap gap-2">
                                let key_id = key
                                    .key
                                    .parse::<super::keys::ApiKeyValue>()
                                    .map_err(anyhow::Error::msg)?;
                                <a
                                    class="border border-stone-300 px-3 py-2 text-sm font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                                    href=(href!(
                                        super::keys::edit_key_route,
                                        super::keys::EditKeyId(key_id),
                                    ))
                                >
                                    "Edit"
                                </a>
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
                                        label: "Delete",
                                        confirmation: delete_confirmation.as_str(),
                                        class: "border border-red-800 px-3 py-2 text-sm font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950"
                                    )
                                </form>
                            </div>
                        </li>
                    }
                </ul>
            }
        </section>
    }
}
