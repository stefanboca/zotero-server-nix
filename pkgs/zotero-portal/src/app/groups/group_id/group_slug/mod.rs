mod members;

use topcoat::context::{Cx, context};
use topcoat::router::error::not_found;
use topcoat::router::{href, page};
use topcoat::view::{View, component, view};
use topcoat::{Error, Result};

use super::group_id;
use crate::app::{authenticated_user, confirm_button, portal_page, section};
use crate::database::Database;
use crate::dataserver::{Dataserver, Group, GroupRole};
use crate::id::UserId;

topcoat::router::module_param!(pub(in crate::app) group_slug);

pub(super) async fn current_slug_group(cx: &Cx) -> Result<(UserId, Group, Option<GroupRole>)> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *group_id(cx)?;
    let group = super::group_by_id(cx, group_id)
        .await?
        .ok_or_else(not_found)?;
    let role = group.role(user_id);
    if role.is_none()
        && context::<Database>(cx)
            .group_invitation(group_id, user_id)
            .await
            .map_err(Error::from_anyhow)?
            .is_none()
    {
        return Err(not_found().into());
    }
    if group_slug(cx) != group.slug() {
        return Err(not_found().into());
    }
    Ok((user_id, group, role))
}

#[page]
pub(in crate::app) async fn group_overview(cx: &Cx) -> Result<impl View> {
    let (_, group, role) = current_slug_group(cx).await?;
    Ok(view! {
        portal_page(
            section(
                <header class="border-b border-stone-200 pb-6">
                    <h1 class="text-3xl font-semibold">(group.name.as_str())</h1>
                    if role.is_some() {
                        <div
                            class="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm"
                        >
                            <a
                                class="portal-link"
                                href=(href!(
                                    crate::app::_web_library::group_library_path,
                                    crate::app::_web_library::library_group_id: group.id,
                                    crate::app::_web_library::library_group_slug: group.slug(),
                                    crate::app::_web_library::library_path: ["items"],
                                ))
                            >
                                "Group Library"
                            </a>
                            if role.is_some_and(GroupRole::can_manage) {
                                <span aria-hidden="true">"·"</span>
                                <a
                                    class="portal-link"
                                    href=(href!(super::group_settings, group_id: group.id))
                                >
                                    "Group Settings"
                                </a>
                            }
                        </div>
                    }
                </header>
                <div class="grid gap-8 py-8 sm:grid-cols-2">
                    <section>
                        <h2 class="text-xl font-semibold">"About"</h2>
                        if group.description.trim().is_empty() {
                            <p class="mt-3 text-stone-600">
                                "No description provided."
                            </p>
                        } else {
                            <p class="mt-3 whitespace-pre-wrap text-stone-600">
                                (group.description.as_str())
                            </p>
                        }
                    </section>
                    group_details(group: &group, role: role)
                </div>
                if role.is_some() {
                    recent_group_items(group: &group)
                } else {
                    <section class="border-t border-stone-200 py-8">
                        <h2 class="text-xl font-semibold">"Recently Added Items"</h2>
                        <p class="mt-3 text-stone-600">
                            "Library will be viewable after joining this group."
                        </p>
                    </section>
                }
            )
        )
    })
}

#[component]
async fn group_details(cx: &Cx, group: &Group, role: Option<GroupRole>) -> Result<impl View> {
    let owner = context::<Database>(cx)
        .user_by_id(group.owner)
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(not_found)?;
    let registered = group.created.strftime("%Y-%m-%d").to_string();
    let leave_confirmation = format!("Leave “{}”?", group.name);
    Ok(view! {
        <section>
            <h2 class="text-xl font-semibold">"Group Details"</h2>
            <dl class="mt-3 grid grid-cols-[8rem_1fr] gap-3 text-sm">
                <dt class="text-stone-600">"Owner"</dt>
                <dd>
                    <a
                        class="portal-link"
                        href=(href!(
                            crate::app::profile::profile,
                            crate::app::profile::profile_username: owner.username.clone(),
                        ))
                    >
                        (owner.display_name())
                    </a>
                </dd>
                <dt class="text-stone-600">"Registered"</dt>
                <dd>
                    <time datetime=(group.created.to_string())>
                        (registered.as_str())
                    </time>
                </dd>
                <dt class="text-stone-600">"Members"</dt>
                <dd>
                    <a
                        class="portal-link"
                        href=(href!(
                            members::members,
                            group_id: group.id,
                            group_slug: group.slug(),
                        ))
                    >
                        (group.user_ids().len())
                    </a>
                </dd>
            </dl>
            if role.is_some_and(|role| role != GroupRole::Owner) {
                <form
                    class="mt-5"
                    method="post"
                    action=(href!(super::members::leave_group, group_id: group.id))
                >
                    confirm_button(
                        label: "Leave Group",
                        confirmation: leave_confirmation.as_str(),
                        class: "portal-button portal-button-danger text-sm"
                    )
                </form>
            }
            if role.is_none() {
                <div class="mt-5">
                    <p class="text-stone-600">
                        "You have been invited to join this group."
                    </p>
                    <div class="mt-3 flex flex-wrap gap-3">
                        <form
                            method="post"
                            action=(href!(super::accept_invitation, group_id: group.id))
                        >
                            <button
                                class="portal-button portal-button-primary"
                                type="submit"
                            >
                                "Join"
                            </button>
                        </form>
                        <form
                            method="post"
                            action=(href!(super::ignore_invitation, group_id: group.id))
                        >
                            <button
                                class="portal-button portal-button-secondary"
                                type="submit"
                            >
                                "Ignore"
                            </button>
                        </form>
                    </div>
                </div>
            }
        </section>
    })
}

#[component]
async fn recent_group_items(cx: &Cx, group: &Group) -> Result<impl View> {
    let recently_added = context::<Dataserver>(cx)
        .recent_group_items(group.id)
        .await
        .map_err(Error::from_anyhow)?;
    let total = recently_added.total;
    Ok(view! {
        <section class="border-t border-stone-200 py-8">
            <h2 class="text-xl font-semibold">"Recently Added Items"</h2>
            if recently_added.items.is_empty() {
                <p class="mt-3 text-stone-600">
                    "There are not yet any items in this collection."
                </p>
            } else {
                <div class="mt-4 overflow-x-auto">
                    <table class="portal-compact-table">
                        <thead>
                            <tr>
                                <th>"Title"</th>
                                <th>"Added By"</th>
                                <th>"Date Modified"</th>
                            </tr>
                        </thead>
                        <tbody>
                            for item in recently_added.items {
                                let title = if item.title.trim().is_empty() {
                                    "Untitled"
                                } else {
                                    item.title.as_str()
                                };
                                let datetime = item.date_modified.to_string();
                                let modified = item
                                    .date_modified
                                    .strftime("%Y-%m-%d %H:%M:%S")
                                    .to_string();
                                <tr>
                                    <td>
                                        let icon = format!(
                                            "/static/web-library/icons/16/item-type/{}@1x.svg",
                                            item.icon_name,
                                        );
                                        <img
                                            class="portal-item-type-icon"
                                            src=(icon)
                                            width="16"
                                            height="16"
                                            alt=""
                                        >
                                        <a
                                            class="portal-link font-medium"
                                            href=(href!(
                                                crate::app::_web_library::group_library_path,
                                                crate::app::_web_library::library_group_id: group.id,
                                                crate::app::_web_library::library_group_slug: group.slug(),
                                                crate::app::_web_library::library_path: [
                                                    "items",
                                                    "itemKey",
                                                    item.key.as_str(),
                                                ],
                                            ))
                                        >
                                            (title)
                                        </a>
                                    </td>
                                    <td class="text-stone-600">
                                        (item.added_by.as_deref().unwrap_or("Unknown"))
                                    </td>
                                    <td class="whitespace-nowrap text-stone-600">
                                        <time datetime=(datetime.as_str())>
                                            (modified.as_str())
                                        </time>
                                    </td>
                                </tr>
                            }
                        </tbody>
                    </table>
                </div>
                <p class="mt-4 text-stone-600">
                    "See all "
                    (total)
                    (if total == 1 { " item" } else { " items" })
                    " for this group in the "
                    <a
                        class="portal-link"
                        href=(href!(
                            crate::app::_web_library::group_library_path,
                            crate::app::_web_library::library_group_id: group.id,
                            crate::app::_web_library::library_group_slug: group.slug(),
                            crate::app::_web_library::library_path: ["items"],
                        ))
                    >
                        "Group Library"
                    </a>
                    "."
                </p>
            }
        </section>
    })
}
