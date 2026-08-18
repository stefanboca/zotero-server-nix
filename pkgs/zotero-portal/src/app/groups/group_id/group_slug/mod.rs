mod members;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, href, page, path_param},
    view::{component, view},
};

use crate::{
    app::{authenticated_user, confirm_button},
    database::Database,
    dataserver::{Dataserver, Group, GroupRole},
    id::UserId,
};

use super::GroupId;

topcoat::router::path_param!(pub(super) group_slug);

pub(super) async fn current_slug_group(cx: &Cx) -> Result<(UserId, Group, GroupRole)> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    let group = app_context::<Dataserver>(cx)
        .group(group_id)
        .await?
        .ok_or_else(not_found)?;
    let role = group.role(user_id).ok_or_else(not_found)?;
    if path_param::<GroupSlug>(cx) != group.slug() {
        return Err(not_found().into());
    }
    Ok((user_id, group, role))
}

#[page]
pub(super) async fn group_overview(cx: &Cx) -> Result {
    let (_, group, role) = current_slug_group(cx).await?;
    let leave_confirmation = format!("Leave “{}”?", group.name);
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div>
                        <h1 class="text-3xl font-semibold">(group.name.as_str())</h1>
                        <p class="mt-2 text-stone-600 dark:text-stone-400">
                            (role.user_summary())
                        </p>
                    </div>
                    <a
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                        href=(href!(
                            crate::app::_web_library::group_library_path,
                            crate::app::_web_library::LibraryGroupId(group.id),
                            crate::app::_web_library::LibraryGroupSlug(group.slug()),
                            crate::app::_web_library::LibraryPath(["items"]),
                        ))
                    >
                        "Group Library"
                    </a>
                </div>
            </header>
            <div class="grid gap-8 py-8 sm:grid-cols-2">
                <section>
                    <h2 class="text-xl font-semibold">"About"</h2>
                    if group.description.trim().is_empty() {
                        <p class="mt-3 text-stone-600 dark:text-stone-400">
                            "No description provided."
                        </p>
                    } else {
                        <p
                            class="mt-3 whitespace-pre-wrap text-stone-600 dark:text-stone-400"
                        >
                            (group.description.as_str())
                        </p>
                    }
                    if !group.url.trim().is_empty() {
                        <p class="mt-3 break-all">
                            <a
                                class="text-red-800 hover:underline dark:text-red-400"
                                href=(group.url.as_str())
                            >
                                (group.url.as_str())
                            </a>
                        </p>
                    }
                </section>
                group_details(group: &group, role: role)
            </div>
            recent_group_items(group: &group)
            <div
                class="flex flex-wrap items-center gap-4 border-t border-stone-200 pt-6 dark:border-stone-800"
            >
                if role.can_manage() {
                    <a
                        class="text-red-800 hover:underline dark:text-red-400"
                        href=(href!(super::group_settings, GroupId(group.id)))
                    >
                        "Group Settings"
                    </a>
                }
                if role != GroupRole::Owner {
                    <form
                        method="post"
                        action=(href!(super::members::leave_group, GroupId(group.id)))
                    >
                        confirm_button(
                            label: "Leave Group",
                            confirmation: leave_confirmation.as_str(),
                            class: "text-red-800 hover:underline dark:text-red-400"
                        )
                    </form>
                }
            </div>
        </main>
    }
}

#[component]
async fn group_details(cx: &Cx, group: &Group, role: GroupRole) -> Result {
    let owner = app_context::<Database>(cx)
        .user_by_id(group.owner)
        .await?
        .ok_or_else(not_found)?;
    let registered = group.created.strftime("%Y-%m-%d").to_string();
    let library_access = if role.can_manage() || group.library_editing == "members" {
        "You can view and edit"
    } else {
        "You can only view"
    };
    view! {
        <section>
            <h2 class="text-xl font-semibold">"Group Details"</h2>
            <dl class="mt-3 grid grid-cols-[8rem_1fr] gap-3 text-sm">
                <dt class="text-stone-600 dark:text-stone-400">"Owner"</dt>
                <dd>
                    <a
                        class="hover:text-red-800 dark:hover:text-red-400"
                        href=(href!(
                            crate::app::profile::profile,
                            crate::app::profile::ProfileUsername(owner.username.clone()),
                        ))
                    >
                        (owner.display_name())
                    </a>
                </dd>
                <dt class="text-stone-600 dark:text-stone-400">"Registered"</dt>
                <dd>
                    <time datetime=(group.created.to_string())>(registered.as_str())</time>
                </dd>
                <dt class="text-stone-600 dark:text-stone-400">"Type"</dt>
                <dd>(group.group_type.as_str())</dd>
                <dt class="text-stone-600 dark:text-stone-400">"Members"</dt>
                <dd>
                    <a
                        class="hover:text-red-800 dark:hover:text-red-400"
                        href=(href!(
                            members::members,
                            GroupId(group.id),
                            GroupSlug(group.slug()),
                        ))
                    >
                        (group.user_ids().len())
                    </a>
                </dd>
                <dt class="text-stone-600 dark:text-stone-400">"Library access"</dt>
                <dd>(library_access)</dd>
            </dl>
        </section>
    }
}

#[component]
async fn recent_group_items(cx: &Cx, group: &Group) -> Result {
    let recently_added = app_context::<Dataserver>(cx)
        .recent_group_items(group.id)
        .await?;
    let total = recently_added.total;
    view! {
        <section class="border-t border-stone-200 py-8 dark:border-stone-800">
            <h2 class="text-xl font-semibold">"Recently Added Items"</h2>
            if recently_added.items.is_empty() {
                <p class="mt-3 text-stone-600 dark:text-stone-400">
                    "There are not yet any items in this collection."
                </p>
            } else {
                <div class="mt-4 overflow-x-auto">
                    <table class="w-full border-collapse text-left text-sm">
                        <thead class="border-y border-stone-300 dark:border-stone-700">
                            <tr>
                                <th class="px-3 py-2 font-semibold">"Title"</th>
                                <th class="px-3 py-2 font-semibold">"Added By"</th>
                                <th class="px-3 py-2 font-semibold">"Date Modified"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-stone-200 dark:divide-stone-800">
                            for item in recently_added.items {
                                let title = if item.title.trim().is_empty() {
                                    "Untitled"
                                } else {
                                    item.title.as_str()
                                };
                                let datetime = item.date_modified.to_string();
                                let modified = format!(
                                    "{} UTC",
                                    datetime.trim_end_matches('Z').replace('T', " "),
                                );
                                <tr>
                                    <td class="px-3 py-3">
                                        <a
                                            class="font-medium hover:text-red-800 dark:hover:text-red-400"
                                            href=(href!(
                                                crate::app::_web_library::group_library_path,
                                                crate::app::_web_library::LibraryGroupId(group.id),
                                                crate::app::_web_library::LibraryGroupSlug(group.slug()),
                                                crate::app::_web_library::LibraryPath([
                                                    "items",
                                                    item.key.as_str(),
                                                ]),
                                            ))
                                        >
                                            (title)
                                        </a>
                                    </td>
                                    <td class="px-3 py-3 text-stone-600 dark:text-stone-400">
                                        (item.added_by.as_deref().unwrap_or("Unknown"))
                                    </td>
                                    <td class="whitespace-nowrap px-3 py-3 text-stone-600 dark:text-stone-400">
                                        <time datetime=(datetime.as_str())>(modified.as_str())</time>
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
                <p class="mt-4 text-stone-600 dark:text-stone-400">
                    "See all "
                    (total)
                    (if total == 1 { " item" } else { " items" })
                    " for this group in the "
                    <a
                        class="text-red-800 hover:underline dark:text-red-400"
                        href=(href!(
                            crate::app::_web_library::group_library_path,
                            crate::app::_web_library::LibraryGroupId(group.id),
                            crate::app::_web_library::LibraryGroupSlug(group.slug()),
                            crate::app::_web_library::LibraryPath(["items"]),
                        ))
                    >
                        "Group Library"
                    </a>
                    "."
                </p>
            }
        </section>
    }
}
