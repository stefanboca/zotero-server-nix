mod group_id;
mod r#new;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{HeaderValue, page, query_params},
    runtime::shard,
    view::{component, view},
};

#[topcoat::router::query_params(error = bad_request)]
struct GroupsQuery {
    notice: Option<String>,
    name: Option<String>,
    dialog: Option<String>,
}

use crate::{
    app::{AppState, confirmation_dialog, signed_in_user, toast_script},
    dataserver::{Group, GroupRole},
    store::UserSummary,
};

#[page]
async fn list_groups(cx: &Cx) -> Result {
    let parameters = query_params::<GroupsQuery>(cx)?;
    let toast = groups_toast(parameters);
    signed_in_user(cx).await?;
    view! {
        groups_index(
            toast: toast.as_ref(),
            open_new: parameters.dialog.as_deref() == Some("new")
        )
    }
}

#[shard]
async fn groups_shard(cx: &Cx, refresh: bool) -> Result {
    let _ = refresh;
    let user = signed_in_user(cx).await?;
    let app = app_context::<AppState>(cx);
    let mut groups = app.backend.groups_for_user(user.user_id).await?;
    groups.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    view! { groups_list(user: &user, groups: &groups) }
}

struct GroupsToast {
    message: String,
    is_error: bool,
}

fn groups_toast(parameters: &GroupsQuery) -> Option<GroupsToast> {
    let name = parameters.name.as_deref().unwrap_or("the group");
    let (message, is_error) = match parameters.notice.as_deref()? {
        "left-group" => (format!("You left “{name}”."), false),
        "group-created" => (format!("Created “{name}”."), false),
        "group-create-failed" => (format!("Couldn’t create “{name}”. Please try again."), true),
        "group-deleted" => (format!("Deleted “{name}”."), false),
        "group-delete-failed" => (format!("Couldn’t delete “{name}”. Please try again."), true),
        _ => return None,
    };
    Some(GroupsToast { message, is_error })
}

pub(super) fn groups_notice_location(notice: &str, name: &str) -> Result<HeaderValue> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", notice)
        .append_pair("name", name)
        .finish();
    HeaderValue::try_from(format!("/groups?{query}")).map_err(Into::into)
}

#[component]
async fn groups_index(toast: Option<&GroupsToast>, open_new: bool) -> Result {
    view! {
        signal groups_refresh = false;

        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div
                class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96"
                aria-live="polite"
                aria-atomic="true"
                data-toast-region=""
            >
                if let Some(toast) = toast {
                    <div
                        class="portal-toast border px-4 py-3 shadow-lg"
                        role=(if toast.is_error { "alert" } else { "status" })
                        data-toast=""
                        data-toast-kind=(if toast.is_error { "error" } else { "success" })
                    >
                        (toast.message.as_str())
                    </div>
                }
            </div>
            <section class="py-8">
                <h1 class="text-2xl font-semibold">"Your groups"</h1>
                <input
                    type="checkbox"
                    hidden="hidden"
                    data-groups-refresh=""
                    @change=$(|event: topcoat::runtime::Event| {
                        groups_refresh.set(event.target.checked)
                    })
                >
                groups_shard(refresh: $(groups_refresh.get()))
                <div
                    class="mt-6 flex justify-end border-t border-stone-200 pt-6 dark:border-stone-800"
                >
                    <form
                        method="post"
                        action="/groups/new"
                        data-confirm="The new private group will allow all members to edit its library and files."
                        data-confirm-title="Create a Zotero group"
                        data-confirm-action="Create group"
                        data-confirm-input-name="name"
                        data-confirm-input-label="Group name"
                        data-confirm-input-required="true"
                        data-confirm-input-maxlength="255"
                        data-confirm-success="Group created."
                        data-confirm-error="Couldn’t create the group. Please try again."
                        data-confirm-auto=(if open_new { "true" } else { "false" })
                        data-confirm-refresh="[data-groups-refresh]"
                        data-confirm-stay="true"
                    >
                        <button
                            class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 dark:bg-red-700"
                            type="submit"
                        >
                            "New group"
                        </button>
                    </form>
                </div>
            </section>
            confirmation_dialog()
            toast_script()
        </main>
    }
}

#[component]
async fn groups_list(user: &UserSummary, groups: &[Group]) -> Result {
    view! {
        if groups.is_empty() {
            <p class="mt-4 text-stone-600 dark:text-stone-400">
                "You do not belong to any groups yet."
            </p>
        } else {
            <ul class="mt-4 divide-y divide-stone-200 dark:divide-stone-800">
                for group in groups {
                    let role = group.role(user.user_id).unwrap_or(GroupRole::Member);
                    let member_count = group.user_ids().len();
                    <li class="py-4">
                        <a
                            class="flex flex-wrap items-center justify-between gap-3 p-3 hover:bg-stone-100 dark:hover:bg-stone-800"
                            href=(format!("/groups/{}", group.id))
                        >
                            <span class="font-medium">(group.name.as_str())</span>
                            <span class="text-sm text-stone-600 dark:text-stone-400">
                                (role.user_summary())
                                " · "
                                (member_count)
                                " "
                                if member_count == 1 {
                                    "member"
                                } else {
                                    "members"
                                }
                            </span>
                        </a>
                    </li>
                }
            </ul>
        }
    }
}
