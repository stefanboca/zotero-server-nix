mod delete;
mod members;
mod owner;
mod rename;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, page, path_param, query_params},
    runtime::{procedure, shard},
    view::{component, view},
};

use crate::{
    app::users::member_search_shard,
    app::{AppState, authenticated_user, confirmation_dialog, toast_script, username_label},
    dataserver::{Group, GroupRole},
    id::UserId,
    store::UserSummary,
};

#[path_param(error = not_found)]
pub(super) struct GroupId(pub(super) u64);

impl GroupId {
    fn from_runtime(value: f64) -> Option<Self> {
        (value.is_finite() && value.fract() == 0.0 && value >= 1.0 && value <= u64::MAX as f64)
            .then_some(Self(value as u64))
    }
}

#[query_params(error = bad_request)]
struct GroupPageQuery {
    notice: Option<String>,
    member: Option<String>,
    role: Option<String>,
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
        "ownership-transfer-failed" => failure(
            "Couldn’t transfer group ownership. Please try again.".to_owned(),
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

pub(super) async fn current_group(cx: &Cx) -> Result<(UserId, Group, GroupRole)> {
    let group_id = *path_param::<GroupId>(cx)?;
    current_group_by_id(cx, group_id).await
}

pub(super) async fn current_group_by_id(
    cx: &Cx,
    group_id: u64,
) -> Result<(UserId, Group, GroupRole)> {
    let user_id = authenticated_user(cx).await?;
    let app = app_context::<AppState>(cx);
    let group = app.backend.group(group_id).await?.ok_or_else(not_found)?;
    let role = group.role(user_id).ok_or_else(not_found)?;
    Ok((user_id, group, role))
}

#[procedure]
async fn update_role_runtime(
    cx: &Cx,
    group_id: f64,
    user_id: f64,
    requested_role: GroupRole,
) -> Result<std::result::Result<GroupRole, String>> {
    let failed = || Err("Couldn’t update the group member role.".to_owned());
    let Some(group_id) = GroupId::from_runtime(group_id) else {
        return Ok(failed());
    };
    let Some(user_id) = UserId::from_runtime(user_id) else {
        return Ok(failed());
    };
    let (_, group, actor_role) = current_group_by_id(cx, group_id.0).await?;
    if !actor_role.can_manage() {
        return Ok(failed());
    }
    let expected_role = match group.role(user_id) {
        Some(GroupRole::Admin) => GroupRole::Member,
        Some(GroupRole::Member) => GroupRole::Admin,
        Some(GroupRole::Owner) | None => return Ok(failed()),
    };
    if requested_role != expected_role {
        return Ok(failed());
    }
    let app = app_context::<AppState>(cx);
    if let Err(error) = app
        .backend
        .set_group_user(group.id, user_id, requested_role)
        .await
    {
        eprintln!("failed to update group member role: {error:#}");
        return Ok(failed());
    }
    Ok(Ok(requested_role))
}

#[procedure]
async fn add_member_runtime(
    cx: &Cx,
    group_id: f64,
    identifier: String,
    requested_role: GroupRole,
) -> Result<std::result::Result<String, String>> {
    let failed = || Err("member-add-failed".to_owned());
    let Some(group_id) = GroupId::from_runtime(group_id) else {
        return Ok(failed());
    };
    if !matches!(requested_role, GroupRole::Admin | GroupRole::Member) {
        return Ok(failed());
    }
    let identifier = identifier.trim();
    let identifier = identifier.strip_prefix('@').unwrap_or(identifier);
    if identifier.is_empty() {
        return Ok(failed());
    }
    let (_, group, actor_role) = current_group_by_id(cx, group_id.0).await?;
    if !actor_role.can_manage() {
        return Ok(failed());
    }
    let app = app_context::<AppState>(cx);
    let Some(target) = app.backend.user_by_identifier(identifier).await? else {
        return Ok(Err("member-not-found".to_owned()));
    };
    if group.role(target.user_id).is_some() {
        return Ok(Err("already-member".to_owned()));
    }
    if let Err(error) = app
        .backend
        .set_group_user(group.id, target.user_id, requested_role)
        .await
    {
        eprintln!("failed to add group member: {error:#}");
        return Ok(failed());
    }
    Ok(Ok(target.username))
}

#[page]
async fn show_group(cx: &Cx) -> Result {
    let parameters = query_params::<GroupPageQuery>(cx)?;
    let toast = toast_notice(parameters);
    let (_, group, role) = current_group(cx).await?;
    view! { group_page(
        group: &group,
        actor_role: role,
        toast: toast.as_ref(),
    ) }
}

#[shard]
async fn members_shard(cx: &Cx, group_id: f64, refresh: bool) -> Result {
    let _ = refresh;
    let group_id = GroupId::from_runtime(group_id).ok_or_else(not_found)?;
    let (actor_id, group, actor_role) = current_group_by_id(cx, group_id.0).await?;
    let users = app_context::<AppState>(cx)
        .backend
        .users_by_ids(&group.user_ids())
        .await?;
    view! { members_list(
        group: &group,
        users: &users,
        actor_id: actor_id,
        actor_role: actor_role,
        manage: None,
    ) }
}

#[component]
async fn group_page(group: &Group, actor_role: GroupRole, toast: Option<&ToastNotice>) -> Result {
    let members_action = format!("/groups/{}/members", group.id);
    let search_results_id = format!("member-search-results-{}", group.id);
    let runtime_group_id = group.id as f64;
    view! {
        signal member_group_id = runtime_group_id;
        signal members_refresh = false;
        signal member_search_query = String::new();
        signal empty_member_search_query = String::new();
        signal member_search_open = false;
        signal new_member_role = GroupRole::Member;
        signal administrator_role = GroupRole::Admin;
        signal member_role = GroupRole::Member;
        signal add_pending = false;
        signal add_toast_visible = false;
        signal add_toast_error = false;
        signal add_toast_outcome = String::new();
        signal add_toast_identity = String::new();
        signal add_toast_role = GroupRole::Member;

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
                    <div class="flex flex-wrap gap-3">
                        if actor_role.can_manage() {
                            <form method="post" action=(format!("/groups/{}/rename", group.id)) data-confirm="Choose a new name for this group." data-confirm-title="Rename group" data-confirm-action="Rename group" data-confirm-input-name="name" data-confirm-input-label="Group name" data-confirm-input-value=(group.name.as_str()) data-confirm-input-required="true" data-confirm-input-maxlength="255" data-confirm-success="Group renamed." data-confirm-error="Couldn’t rename the group. Please try again.">
                                <button class="border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" type="submit">"Rename group"</button>
                            </form>
                        }
                        if actor_role == GroupRole::Owner {
                            <form method="post" action=(format!("/groups/{}/delete", group.id)) data-confirm=(format!("Permanently delete “{}” and its synced items and files?", group.name)) data-confirm-action="Delete group" data-confirm-input-name="confirmation" data-confirm-input-label="Group name" data-confirm-input-match=(group.name.as_str()) data-confirm-success=(format!("Deleted {}.", group.name)) data-confirm-error=(format!("Couldn’t delete {}. Please try again.", group.name))>
                                <button class="border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950" type="submit">"Delete group"</button>
                            </form>
                        } else {
                            <form method="post" action=(members_action.as_str()) data-confirm="Leave this group?" data-confirm-action="Leave group" data-confirm-success="You left the group." data-confirm-error="Couldn’t leave the group. Please try again.">
                                <input type="hidden" name="operation" value="leave">
                                <button class="border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950" type="submit">"Leave group"</button>
                            </form>
                        }
                    </div>
                </div>
            </section>

            <input type="checkbox" hidden="hidden" data-members-refresh="" @change=$(|event: topcoat::runtime::Event| members_refresh.set(event.target.checked))>
            members_shard(
                group_id: $(member_group_id.get()),
                refresh: $(members_refresh.get()),
            )

            if actor_role.can_manage() {
                <section class="py-8">
                    <h2 class="text-xl font-semibold">"Add a member"</h2>
                    <p class="mt-2 text-stone-600 dark:text-stone-400">
                        "Search by name, username, or email. You can also enter an exact username or email directly. The person must have signed in to this server at least once."
                    </p>
                    <form class="mt-5 grid grid-cols-[minmax(0,1fr)_5.5rem] items-center gap-3 sm:grid-cols-[minmax(0,1fr)_13rem_5.5rem] sm:gap-4" data-member-picker="" @submit=$(async |event: topcoat::runtime::Event| {
                        event.prevent_default();
                        let requested_identifier = member_search_query.get().trim().to_owned();
                        add_pending.set(true);
                        add_toast_identity.set(requested_identifier.to_owned());
                        add_toast_role.set(new_member_role.get());
                        let outcome = add_member_runtime(member_group_id.get(), requested_identifier, new_member_role.get()).await;
                        if outcome.is_ok() {
                            let added = outcome.unwrap();
                            add_toast_identity.set(added);
                            add_toast_error.set(false);
                            member_search_query.set(empty_member_search_query.get());
                            member_search_open.set(false);
                            members_refresh.toggle();
                        } else {
                            add_toast_outcome.set(outcome.unwrap_err());
                            add_toast_error.set(true);
                        }
                        add_pending.set(false);
                        add_toast_visible.set(true);
                    })>
                        <div class="relative col-span-2 sm:col-span-1">
                            <label class="sr-only" for="member-identifier">"Name, username, or email"</label>
                            <input type="checkbox" hidden="hidden" data-member-search-open="" @change=$(|event: topcoat::runtime::Event| member_search_open.set(event.target.checked))>
                            <input class="w-full rounded border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950" id="member-identifier" type="search" name="identifier" placeholder="Name, username, or email" required="required" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls=(search_results_id.as_str()) data-member-search="" :value=$(member_search_query.get()) @input=$(|event: topcoat::runtime::Event| member_search_query.set(event.target.value))>
                            <div class="absolute z-10 mt-1 max-h-96 w-full overflow-y-auto border border-stone-300 bg-white shadow-lg dark:border-stone-700 dark:bg-stone-950" id=(search_results_id) role="listbox" aria-live="polite" data-member-search-results="" :hidden=$(!member_search_open.get())>
                                member_search_shard(query: $(member_search_query.get()))
                            </div>
                        </div>
                        <input class="administrator-checkbox sr-only" id="new-member-administrator" type="checkbox" name="role" value="admin" :checked=$(new_member_role.get() == administrator_role.get()) @change=$(|event: topcoat::runtime::Event| new_member_role.set(if event.target.checked { administrator_role.get() } else { member_role.get() }))>
                        <label class="role-toggle flex w-full cursor-pointer items-center justify-between gap-3 border border-stone-300 px-3 py-2 text-left hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" for="new-member-administrator" :data-current-role=$(if new_member_role.get() == administrator_role.get() { "admin" } else { "member" })>
                            <span class="text-sm font-medium">"Administrator"</span>
                            <span class="flex items-center gap-2" aria-hidden="true">
                                <span class="role-toggle-track"><span class="role-toggle-knob"></span></span>
                                <span class="w-6 text-xs font-semibold uppercase tracking-wide">$(if new_member_role.get() == administrator_role.get() { "On" } else { "Off" })</span>
                            </span>
                        </label>
                        <button class="w-full rounded border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-stone-700 dark:hover:bg-stone-800" type="submit" :disabled=$(add_pending.get())>
                            "Add"
                        </button>
                    </form>
                    <div class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96" aria-live="polite" aria-atomic="true">
                        <div class="portal-toast border px-4 py-3 shadow-lg" :hidden=$(!add_toast_visible.get()) :role=$(if add_toast_error.get() { "alert" } else { "status" }) :data-toast-kind=$(if add_toast_error.get() { "error" } else { "success" }) @animationend=$(|_event: topcoat::runtime::Event| add_toast_visible.set(false))>
                            $(if add_toast_error.get() {
                                if add_toast_outcome.get() == "member-not-found" { "Couldn’t add " } else { "" }
                            } else { "" })
                            <strong class="font-mono font-semibold">$(add_toast_identity.get())</strong>
                            $(if add_toast_error.get() {
                                if add_toast_outcome.get() == "member-not-found" {
                                    ". Please check the username or email and try again."
                                } else if add_toast_outcome.get() == "already-member" {
                                    " is already a member of this group."
                                } else {
                                    " couldn’t be added to the group. Please try again."
                                }
                            } else if add_toast_role.get() == administrator_role.get() {
                                " added as Administrator."
                            } else {
                                " added as Member."
                            })
                        </div>
                    </div>
                </section>
            }
            confirmation_dialog()
            <link rel="stylesheet" href=(topcoat::asset::asset!("src/app/groups/group_id/group.css"))>
            toast_script()
            <script type="module" src=(topcoat::asset::asset!("src/app/groups/group_id/group_actions.js"))></script>
            <script type="module" src=(topcoat::asset::asset!("src/app/groups/group_id/member_search.js"))></script>
        </main>
    }
}

#[component]
pub(super) async fn members_list(
    group: &Group,
    users: &[UserSummary],
    actor_id: UserId,
    actor_role: GroupRole,
    manage: Option<&str>,
) -> Result {
    let members_action = format!("/groups/{}/members", group.id);
    let mut ordered_users = users.iter().collect::<Vec<_>>();
    ordered_users.sort_by_key(|member| member.user_id != actor_id);
    view! {
        <section class="border-b border-stone-200 py-8 dark:border-stone-800" data-members-region="">
            <h2 class="text-xl font-semibold">"Members"</h2>
            <ul class="mt-5 divide-y divide-stone-200 dark:divide-stone-800">
                for member in ordered_users {
                    let member_role = group.role(member.user_id).unwrap_or(GroupRole::Member);
                    <li class="grid grid-cols-[minmax(0,1fr)_5.5rem] items-center gap-3 py-4 sm:grid-cols-[minmax(0,1fr)_13rem_5.5rem] sm:gap-4" data-confirm-item="">
                        <div class="col-span-2 min-w-0 sm:col-span-1">
                            <p class="truncate font-medium">(member.display_name())</p>
                            <p class="truncate text-sm text-stone-600 dark:text-stone-400">
                                username_label(username: member.username.as_str()) " · " (member.email.as_str())
                            </p>
                        </div>
                        if actor_role.can_manage() && member_role != GroupRole::Owner {
                            if member.user_id == actor_id {
                                administrator_self_toggle(group_id: group.id)
                            } else {
                                administrator_toggle(role: member_role, group_id: group.id, user_id: member.user_id, username: member.username.as_str())
                            }
                        } else {
                            <span class="text-sm text-stone-600 dark:text-stone-400">(member_role.display_name())</span>
                        }
                        if actor_role.can_manage() && member_role != GroupRole::Owner && member.user_id != actor_id {
                            <details class="relative" open=(manage == Some(member.username.as_str())) data-manage-menu="">
                                <summary class="w-full cursor-pointer list-none border border-stone-300 px-3 py-2 text-center text-sm hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-800">"Manage"</summary>
                                <div class="absolute right-0 z-10 mt-2 w-72 max-w-[calc(100vw_-_3rem)] space-y-3 border border-stone-300 bg-white p-4 shadow-lg dark:border-stone-700 dark:bg-stone-950">
                                    if actor_role == GroupRole::Owner {
                                        <form method="post" action=(format!("/groups/{}/owner", group.id)) data-confirm=(format!("Transfer “{}” to {}? You will become an administrator.", group.name, member.username)) data-confirm-action="Transfer ownership" data-confirm-input-name="confirmation" data-confirm-input-label="Group name" data-confirm-input-match=(group.name.as_str()) data-confirm-success="Group ownership transferred." data-confirm-error="Couldn’t transfer group ownership. Please try again.">
                                            <input type="hidden" name="username" value=(member.username.as_str())>
                                            <button class="text-sm font-medium hover:text-red-800 dark:hover:text-red-400" type="submit">"Transfer ownership"</button>
                                        </form>
                                    }
                                    <form class=(if actor_role == GroupRole::Owner { "border-t border-stone-200 pt-3 dark:border-stone-800" } else { "" }) method="post" action=(members_action.as_str()) data-confirm="Remove this member from the group?" data-confirm-action="Remove member" data-confirm-success-prefix="Removed " data-confirm-success-username=(member.username.as_str()) data-confirm-success-suffix=" from the group." data-confirm-error-prefix="Couldn’t remove " data-confirm-error-username=(member.username.as_str()) data-confirm-error-suffix=" from the group. Please try again." data-confirm-refresh="[data-members-refresh]">
                                        <input type="hidden" name="operation" value="remove">
                                        <input type="hidden" name="identifier" value=(member.username.as_str())>
                                        <button class="text-sm font-medium text-red-800 hover:underline dark:text-red-400" type="submit">"Remove from group"</button>
                                    </form>
                                </div>
                            </details>
                        } else {
                            <span aria-hidden="true"></span>
                        }
                    </li>
                }
            </ul>
        </section>
    }
}

#[component]
async fn administrator_self_toggle(group_id: u64) -> Result {
    view! {
        <form class="w-full" method="post" action=(format!("/groups/{group_id}/members")) data-confirm="Become a member of this group? You will no longer be able to manage the group or its members." data-confirm-title="Change your role" data-confirm-action="Become a member" data-confirm-success="You are now a group member." data-confirm-error="Couldn’t change your role. Please try again.">
            <input type="hidden" name="operation" value="demote-self">
            <button class="role-toggle flex w-full items-center justify-between gap-3 border border-stone-300 px-3 py-2 text-left hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" type="submit" data-current-role="admin" aria-pressed="true" aria-label="Administrator is on. Toggle off.">
                <span class="text-sm font-medium">"Administrator"</span>
                <span class="flex items-center gap-2" aria-hidden="true">
                    <span class="role-toggle-track"><span class="role-toggle-knob"></span></span>
                    <span class="w-6 text-xs font-semibold uppercase tracking-wide">"On"</span>
                </span>
            </button>
        </form>
    }
}

#[component]
async fn administrator_toggle(
    role: GroupRole,
    group_id: u64,
    user_id: UserId,
    username: &str,
) -> Result {
    let runtime_group_id = group_id as f64;
    let runtime_user_id = user_id.get() as f64;
    view! {
        signal current_role = role;
        signal administrator_role = GroupRole::Admin;
        signal member_role = GroupRole::Member;
        signal member_group_id = runtime_group_id;
        signal member_user_id = runtime_user_id;
        signal toast_visible = false;
        signal toast_error = false;

        <button class="role-toggle flex w-full items-center justify-between gap-3 border border-stone-300 px-3 py-2 text-left hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900" type="button" :data-current-role=$(if current_role.get() == administrator_role.get() { "admin" } else { "member" }) :aria-pressed=$(if current_role.get() == administrator_role.get() { "true" } else { "false" }) :aria-label=$(if current_role.get() == administrator_role.get() { "Administrator is on. Toggle off." } else { "Administrator is off. Toggle on." }) data-role-toggle="" @click=$(async |event: topcoat::runtime::Event| {
                event.prevent_default();
                let next_role = if current_role.get() == administrator_role.get() { member_role.get() } else { administrator_role.get() };
                let outcome = update_role_runtime(member_group_id.get(), member_user_id.get(), next_role).await;
                if outcome.is_ok() {
                    current_role.set(outcome.unwrap());
                    toast_error.set(false);
                } else {
                    toast_error.set(true);
                }
                toast_visible.set(true);
            })>
                <span class="text-sm font-medium">"Administrator"</span>
                <span class="flex items-center gap-2" aria-hidden="true">
                    <span class="role-toggle-track"><span class="role-toggle-knob"></span></span>
                    <span class="w-6 text-xs font-semibold uppercase tracking-wide" data-role-state="">$(if current_role.get() == administrator_role.get() { "On" } else { "Off" })</span>
                </span>
        </button>
        <div class="pointer-events-none fixed inset-x-4 top-20 z-50 sm:left-auto sm:right-5 sm:w-96" aria-live="polite" aria-atomic="true">
            <div class="portal-toast border px-4 py-3 shadow-lg" :hidden=$(!toast_visible.get()) :role=$(if toast_error.get() { "alert" } else { "status" }) :data-toast-kind=$(if toast_error.get() { "error" } else { "success" }) @animationend=$(|_event: topcoat::runtime::Event| toast_visible.set(false))>
                    $(if toast_error.get() { "Couldn’t update " } else { "" })
                    <strong class="font-mono font-semibold">(username)</strong>
                    $(if toast_error.get() { " to " } else { " is now " })
                    $(if toast_error.get() {
                        if current_role.get() == administrator_role.get() { "Member" } else { "Administrator" }
                    } else if current_role.get() == administrator_role.get() {
                        "Administrator"
                    } else {
                        "Member"
                    })
                    $(if toast_error.get() { ". Please try again." } else { "." })
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use topcoat::context::Cx;
    use topcoat::router::{Body, Request, StatusCode};

    use super::*;

    #[tokio::test]
    async fn role_procedure_is_registered_on_the_application_router() {
        let procedure_id =
            serde_json::to_string(&update_role_runtime.id()).expect("procedure ID must serialize");
        let procedure_id = procedure_id.trim_matches('"');
        let request = Request::post(format!("/_topcoat/procedures/{procedure_id}"))
            .body(Body::empty())
            .expect("procedure request must be valid");

        let response = crate::app::routes().build().handle(request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_member_procedure_is_registered_on_the_application_router() {
        let procedure_id =
            serde_json::to_string(&add_member_runtime.id()).expect("procedure ID must serialize");
        let procedure_id = procedure_id.trim_matches('"');
        let request = Request::post(format!("/_topcoat/procedures/{procedure_id}"))
            .body(Body::empty())
            .expect("procedure request must be valid");

        let response = crate::app::routes().build().handle(request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn role_toggle_renders_a_runtime_click_binding() {
        let cx = Cx::default();
        let __cx = &cx;
        let view: Result = view! {
            administrator_toggle(
                role: GroupRole::Admin,
                group_id: 1,
                user_id: UserId::new(2),
                username: "member"
            )
        };
        let html = view.expect("role toggle must render").render(__cx);

        assert!(html.contains("data-topcoat-on:click="), "{html}");
        assert!(html.contains("data-current-role=\"admin\""), "{html}");
        assert!(html.contains("aria-pressed=\"true\""), "{html}");
        assert!(
            html.contains("&quot;t&quot;:&quot;Procedure&quot;"),
            "{html}"
        );
    }

    #[tokio::test]
    async fn self_role_toggle_uses_the_confirmation_dialog() {
        let cx = Cx::default();
        let __cx = &cx;
        let view: Result = view! { administrator_self_toggle(group_id: 2) };
        let html = view.expect("self role toggle must render").render(__cx);

        assert!(html.contains("data-confirm="), "{html}");
        assert!(html.contains("value=\"demote-self\""), "{html}");
        assert!(html.contains("aria-pressed=\"true\""), "{html}");
        assert!(!html.contains("data-topcoat-on:click="), "{html}");
    }
}
