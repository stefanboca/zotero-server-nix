mod delete;
mod members;
mod owner;
mod rename;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, page, path_param, query_params},
    view::{component, view},
};

use crate::{
    app::{AppState, authenticated_user, is_topcoat_dev, username_label},
    dataserver::{Group, GroupRole},
    store::{self, UserSummary},
};

#[path_param(error = not_found)]
pub(super) struct GroupId(pub(super) u64);

#[query_params(error = topcoat::router::error::bad_request)]
struct GroupPageQuery {
    notice: Option<String>,
    member: Option<String>,
    role: Option<String>,
    manage: Option<String>,
    name: Option<String>,
}

struct ToastNotice {
    leading: String,
    username: Option<String>,
    trailing: String,
    is_error: bool,
}

fn toast_notice(parameters: &GroupPageQuery) -> Option<ToastNotice> {
    let member = parameters.member.as_deref();
    let role = parameters.role.as_deref();
    let success = |leading, username, trailing| {
        Some(ToastNotice {
            leading,
            username,
            trailing,
            is_error: false,
        })
    };
    let failure = |leading, username, trailing| {
        Some(ToastNotice {
            leading,
            username,
            trailing,
            is_error: true,
        })
    };

    match parameters.notice.as_deref()? {
        "member-added" => success(
            "Added ".to_owned(),
            member.map(str::to_owned),
            format!(" as {}.", role.unwrap_or("Member")),
        ),
        "member-add-failed" => failure(
            "Couldn’t add ".to_owned(),
            member.map(str::to_owned),
            format!(" as {}. Please try again.", role.unwrap_or("a member")),
        ),
        "member-not-found" => failure(
            format!(
                "No account matched {}. Enter an exact username or email.",
                member.unwrap_or("that value")
            ),
            None,
            String::new(),
        ),
        "already-member" => failure(
            String::new(),
            member.map(str::to_owned),
            " is already a member of this group.".to_owned(),
        ),
        "role-updated" => success(
            String::new(),
            member.map(str::to_owned),
            format!(" is now {}.", role.unwrap_or("a member")),
        ),
        "role-update-failed" => failure(
            "Couldn’t update ".to_owned(),
            member.map(str::to_owned),
            format!(" to {}. Please try again.", role.unwrap_or("that role")),
        ),
        "member-removed" => success(
            "Removed ".to_owned(),
            member.map(str::to_owned),
            " from the group.".to_owned(),
        ),
        "member-remove-failed" => failure(
            "Couldn’t remove ".to_owned(),
            member.map(str::to_owned),
            " from the group. Please try again.".to_owned(),
        ),
        "ownership-transferred" => success(
            "Group ownership transferred.".to_owned(),
            None,
            String::new(),
        ),
        "group-renamed" => success(
            format!(
                "Group renamed to “{}”.",
                parameters.name.as_deref().unwrap_or("the new name")
            ),
            None,
            String::new(),
        ),
        "group-rename-failed" => failure(
            "Couldn’t rename the group. Please try again.".to_owned(),
            None,
            String::new(),
        ),
        _ => None,
    }
}

fn preview_group(group_id: u64) -> Group {
    Group {
        id: group_id,
        name: "Research group".to_owned(),
        owner: 1,
        admins: vec![2],
        members: vec![3],
        group_type: "Private".to_owned(),
        library_editing: "members".to_owned(),
        library_reading: "members".to_owned(),
        file_editing: "members".to_owned(),
        description: String::new(),
        url: String::new(),
        has_image: false,
    }
}

pub(super) async fn current_group(cx: &Cx) -> Result<(u64, Group, GroupRole)> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    if is_topcoat_dev() {
        return Ok((user_id, preview_group(group_id), GroupRole::Owner));
    }
    let app = app_context::<AppState>(cx);
    let group = app
        .dataserver
        .group(group_id)
        .await?
        .ok_or_else(not_found)?;
    let role = group.role(user_id).ok_or_else(not_found)?;
    Ok((user_id, group, role))
}

#[page]
async fn show_group(cx: &Cx) -> Result {
    let parameters = query_params::<GroupPageQuery>(cx)?;
    let toast = toast_notice(parameters);
    if is_topcoat_dev() {
        let sample = preview_group(*path_param::<GroupId>(cx)?);
        let users = vec![
            UserSummary {
                user_id: 1,
                username: "owner".to_owned(),
                email: "owner@example.com".to_owned(),
                real_name: Some("Group Owner".to_owned()),
            },
            UserSummary {
                user_id: 2,
                username: "admin".to_owned(),
                email: "admin@example.com".to_owned(),
                real_name: Some("Group Admin".to_owned()),
            },
            UserSummary {
                user_id: 3,
                username: "member".to_owned(),
                email: "member@example.com".to_owned(),
                real_name: Some("Group Member".to_owned()),
            },
        ];
        return view! { group_page(
            group: &sample,
            users: &users,
            actor_id: 1,
            actor_role: GroupRole::Owner,
            toast: toast.as_ref(),
            manage: parameters.manage.as_deref(),
        ) };
    }

    let (user_id, group, role) = current_group(cx).await?;
    let app = app_context::<AppState>(cx);
    let users = store::users_by_ids(&app.database, &group.user_ids()).await?;
    view! { group_page(
        group: &group,
        users: &users,
        actor_id: user_id,
        actor_role: role,
        toast: toast.as_ref(),
        manage: parameters.manage.as_deref(),
    ) }
}

#[component]
async fn group_page(
    group: &Group,
    users: &[UserSummary],
    actor_id: u64,
    actor_role: GroupRole,
    toast: Option<&ToastNotice>,
    manage: Option<&str>,
) -> Result {
    let members_action = format!("/groups/{}/members", group.id);
    let delete_url = format!("/groups/{}/delete", group.id);
    let search_url = "/users/search";
    let search_results_id = format!("member-search-results-{}", group.id);
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96" aria-live="polite" aria-atomic="true" data-toast-region="">
                if let Some(toast) = toast {
                    <div class="portal-toast border px-4 py-3 shadow-lg" role=(if toast.is_error { "alert" } else { "status" }) data-toast="" data-toast-kind=(if toast.is_error { "error" } else { "success" })>
                        (toast.leading.as_str())
                        if let Some(username) = toast.username.as_deref() {
                            username_label(username: username)
                        }
                        (toast.trailing.as_str())
                    </div>
                }
            </div>
            <section class="border-b border-stone-200 py-8 dark:border-stone-800">
                <div class="flex flex-wrap items-center justify-between gap-4">
                    <div>
                        <h1 class="text-2xl font-semibold">(group.name.as_str())</h1>
                        <p class="mt-1 text-stone-600 dark:text-stone-400">
                            (actor_role.user_summary())
                        </p>
                    </div>
                    if actor_role.can_manage() {
                        <div class="flex flex-wrap gap-3">
                            <a class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" href=(format!("/groups/{}/rename", group.id))>
                                "Rename group"
                            </a>
                            if actor_role == GroupRole::Owner {
                                <a class="border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950" href=(delete_url)>
                                    "Delete group"
                                </a>
                            }
                        </div>
                    }
                </div>
            </section>

            <section class="border-b border-stone-200 py-8 dark:border-stone-800">
                <h2 class="text-xl font-semibold">"Members"</h2>
                <ul class="mt-5 divide-y divide-stone-200 dark:divide-stone-800">
                    for member in users {
                        let member_role = group.role(member.user_id).unwrap_or(GroupRole::Member);
                        <li class="flex flex-col items-stretch gap-3 py-4 sm:flex-row sm:items-center sm:justify-between sm:gap-5">
                            <div class="min-w-0">
                                <p class="truncate font-medium">(member.display_name())</p>
                                <p class="truncate text-sm text-stone-600 dark:text-stone-400">
                                    username_label(username: member.username.as_str()) " · " (member.email.as_str())
                                </p>
                            </div>
                            <div class="flex shrink-0 items-center justify-between gap-4 sm:justify-start">
                                <span class="text-sm text-stone-600 dark:text-stone-400" data-member-role-label="">
                                    (member_role.display_name())
                                </span>
                                if actor_role.can_manage() && member_role != GroupRole::Owner {
                                    <details class="relative" open=(manage == Some(member.username.as_str())) data-manage-menu="">
                                        <summary class="cursor-pointer list-none border border-stone-300 px-3 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-800">
                                            "Manage"
                                        </summary>
                                        <div class="absolute right-0 z-10 mt-2 w-72 max-w-[calc(100vw_-_3rem)] space-y-3 border border-stone-300 bg-white p-4 shadow-lg dark:border-stone-700 dark:bg-stone-950">
                                            <form method="post" action=(members_action.as_str()) data-role-form="">
                                                <input type="hidden" name="operation" value="role">
                                                <input type="hidden" name="identifier" value=(member.username.as_str())>
                                                <button class="role-toggle flex w-full items-center justify-between gap-4 border border-stone-300 px-3 py-2 text-left hover:bg-stone-100 disabled:cursor-wait disabled:opacity-60 dark:border-stone-700 dark:hover:bg-stone-900" type="submit" name="role" value=(if member_role == GroupRole::Admin { "member" } else { "admin" }) aria-pressed=(if member_role == GroupRole::Admin { "true" } else { "false" }) aria-label=(if member_role == GroupRole::Admin { "Administrator is on. Toggle off." } else { "Administrator is off. Toggle on." }) data-current-role=(member_role.as_str()) data-role-toggle="">
                                                    <span class="text-sm font-medium">"Administrator"</span>
                                                    <span class="flex items-center gap-2" aria-hidden="true">
                                                        <span class="role-toggle-track"><span class="role-toggle-knob"></span></span>
                                                        <span class="w-6 text-xs font-semibold uppercase tracking-wide" data-role-state="">(if member_role == GroupRole::Admin { "On" } else { "Off" })</span>
                                                    </span>
                                                </button>
                                            </form>
                                            if actor_role == GroupRole::Owner {
                                                let transfer_query = url::form_urlencoded::Serializer::new(String::new())
                                                    .append_pair("username", &member.username)
                                                    .finish();
                                                <a class="block border-t border-stone-200 pt-3 text-sm font-medium hover:text-red-800 dark:border-stone-800 dark:hover:text-red-400" href=(format!("/groups/{}/owner?{transfer_query}", group.id))>
                                                    "Transfer ownership"
                                                </a>
                                            }
                                            <form class="border-t border-stone-200 pt-3 dark:border-stone-800" method="post" action=(members_action.as_str()) data-confirm="Remove this member from the group?" data-confirm-action="Remove member">
                                                <input type="hidden" name="operation" value="remove">
                                                <input type="hidden" name="identifier" value=(member.username.as_str())>
                                                <button class="text-sm font-medium text-red-800 hover:underline dark:text-red-400" type="submit">
                                                    "Remove from group"
                                                </button>
                                            </form>
                                        </div>
                                    </details>
                                } else if member.user_id == actor_id && member_role != GroupRole::Owner {
                                    <details class="relative" data-manage-menu="">
                                        <summary class="cursor-pointer list-none border border-stone-300 px-3 py-2 text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-800">
                                            "Manage"
                                        </summary>
                                        <div class="absolute right-0 z-10 mt-2 w-56 max-w-[calc(100vw_-_3rem)] border border-stone-300 bg-white p-4 shadow-lg dark:border-stone-700 dark:bg-stone-950">
                                            <form method="post" action=(members_action.as_str()) data-confirm="Leave this group?" data-confirm-action="Leave group">
                                                <input type="hidden" name="operation" value="remove">
                                                <input type="hidden" name="identifier" value=(member.username.as_str())>
                                                <button class="text-sm font-medium text-red-800 hover:underline dark:text-red-400" type="submit">
                                                    "Leave group"
                                                </button>
                                            </form>
                                        </div>
                                    </details>
                                }
                            </div>
                        </li>
                    }
                </ul>
            </section>

            if actor_role.can_manage() {
                <section class="py-8">
                    <h2 class="text-xl font-semibold">"Add a member"</h2>
                    <p class="mt-2 text-stone-600 dark:text-stone-400">
                        "Search by name, username, or email. You can also enter an exact username or email directly. The person must have signed in to this server at least once."
                    </p>
                    <form class="mt-5 grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto_auto]" method="post" action=(members_action) data-member-picker="">
                        <input type="hidden" name="operation" value="add">
                        <div class="relative">
                            <label class="sr-only" for="member-identifier">"Name, username, or email"</label>
                            <input class="w-full rounded border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950" id="member-identifier" type="search" name="identifier" placeholder="Name, username, or email" required="required" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls=(search_results_id.as_str()) data-member-search="" data-search-url=(search_url)>
                            <div class="absolute z-10 mt-1 max-h-96 w-full overflow-y-auto border border-stone-300 bg-white shadow-lg dark:border-stone-700 dark:bg-stone-950" id=(search_results_id) role="listbox" aria-live="polite" data-member-search-results="" hidden="hidden"></div>
                        </div>
                        <select class="rounded border border-stone-300 bg-white px-2 py-2 dark:border-stone-700 dark:bg-stone-950" name="role" aria-label="New member role">
                            <option value="member">"Member"</option>
                            <option value="admin">"Administrator"</option>
                        </select>
                        <button class="rounded border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-800" type="submit">
                            "Add"
                        </button>
                    </form>
                </section>
            }
            <dialog class="fixed inset-0 m-auto max-h-[calc(100vh_-_2rem)] w-[calc(100%_-_2rem)] max-w-lg overflow-y-auto border border-stone-300 bg-white p-0 text-stone-950 shadow-xl backdrop:bg-stone-950/60 dark:border-stone-700 dark:bg-stone-950 dark:text-stone-50" data-confirm-dialog="">
                <div class="p-6">
                    <h2 class="text-xl font-semibold">"Confirm action"</h2>
                    <p class="mt-3 leading-7 text-stone-600 dark:text-stone-400" data-confirm-message=""></p>
                    <div class="mt-6 flex flex-wrap justify-end gap-3">
                        <button class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-800" type="button" data-confirm-submit="">"Remove"</button>
                        <button class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700" type="button" data-confirm-cancel="">"Cancel"</button>
                    </div>
                </div>
            </dialog>
            <link rel="stylesheet" href=(topcoat::asset::asset!("src/app/groups/group_id/group.css"))>
            <script type="module" src=(topcoat::asset::asset!("src/app/toast.js"))></script>
            <script type="module" src=(topcoat::asset::asset!("src/app/groups/group_id/group_actions.js"))></script>
            <script type="module" src=(topcoat::asset::asset!("src/app/groups/group_id/member_search.js"))></script>
        </main>
    }
}
