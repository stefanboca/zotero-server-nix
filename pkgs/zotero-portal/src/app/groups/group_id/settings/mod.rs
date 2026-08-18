mod library;
mod members;

pub(in crate::app::groups) use library::library as library_settings;
pub(in crate::app::groups) use members::members as members_settings;

use serde::{Deserialize, Serialize};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{bad_request, forbidden, not_found, see_other},
        href, layout, path_param, query_params,
        request::uri,
        response::{IntoResponse, Response},
        route,
    },
    view::{PromotedStr, view},
};

use crate::{
    app::{
        SidebarItem, confirm_button, groups::GroupName, portal_page, section, sidebar,
        success_message,
    },
    database::Database,
    dataserver::{Dataserver, Group, GroupRole},
    id::UserId,
};

use super::{GroupId, current_group, group_by_id};

#[derive(Clone)]
pub(super) struct GroupSettingsContext {
    pub(super) group: Group,
    pub(super) actor_id: UserId,
    pub(super) actor_role: GroupRole,
}

pub(super) async fn managed_group_settings(cx: &Cx) -> Result<GroupSettingsContext> {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    if !actor_role.can_manage() {
        return Err(forbidden().into());
    }
    Ok(GroupSettingsContext {
        group,
        actor_id,
        actor_role,
    })
}

#[layout]
async fn group_settings_layout(cx: &Cx, slot: Result) -> Result {
    let slot = slot?;
    let path = uri(cx).path();
    let group_id = *path_param::<GroupId>(cx)?;
    let group = group_by_id(cx, group_id).await?.ok_or_else(not_found)?;
    let general = href!(settings, GroupId(group_id)).resolve(cx);
    let library = href!(library::library, GroupId(group_id)).resolve(cx);
    let members = href!(members::members, GroupId(group_id)).resolve(cx);
    let navigation = vec![
        SidebarItem {
            active: path == general,
            href: general,
            label: PromotedStr(&"Group"),
        },
        SidebarItem {
            active: path == members,
            href: members,
            label: PromotedStr(&"Members"),
        },
        SidebarItem {
            active: path == library,
            href: library,
            label: PromotedStr(&"Library"),
        },
    ];
    let group_href = href!(
        super::group_slug::group_overview,
        GroupId(group.id),
        super::group_slug::GroupSlug(group.slug()),
    )
    .resolve(cx);
    view! {
        portal_page(
            <div class="portal-settings-heading-row">
                <h1 class="portal-settings-heading break-words">
                    <a href=(group_href)>(group.name)</a>
                </h1>
            </div>
            <div class="portal-settings-layout">
                sidebar(
                    aria_label: PromotedStr(&"Group settings sections"),
                    items: navigation
                )
                <div class="min-w-0">(slot)</div>
            </div>
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDetails {
    name: GroupName,
    description: String,
}

#[topcoat::router::query_params(error = bad_request)]
#[derive(Serialize)]
struct SettingsQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    saved: Option<bool>,
}

#[topcoat::router::page]
pub(in crate::app::groups) async fn settings(cx: &Cx) -> Result {
    let context = managed_group_settings(cx).await?;
    let query = query_params::<SettingsQuery>(cx)?;
    let transfer_candidates = if context.actor_role == GroupRole::Owner {
        let mut users = app_context::<Database>(cx)
            .users_by_ids(&context.group.user_ids())
            .await?;
        users.retain(|user| user.user_id != context.group.owner);
        Some(users)
    } else {
        None
    };
    view! {
        if query.saved == Some(true) {
            success_message(message: "Group Settings Saved")
        }
        <div class="space-y-6">
            section(
                <h2 class="portal-section-heading">"Group Information"</h2>
                <form
                    class="mt-5 space-y-6"
                    method="post"
                    action=(href!(settings, GroupId(context.group.id)))
                >
                    <div>
                        <label class="block font-medium" for="group-name">"Name"</label>
                        <input
                            class="mt-2 w-full border border-stone-300 bg-white px-3 py-2"
                            id="group-name"
                            name="name"
                            value=(context.group.name.as_str())
                            required="required"
                            maxlength="255"
                        >
                    </div>
                    <div>
                        <label class="block font-medium" for="group-description">
                            "Description"
                        </label>
                        <textarea
                            class="mt-2 min-h-32 w-full border border-stone-300 bg-white px-3 py-2"
                            id="group-description"
                            name="description"
                            maxlength="10000"
                        >
                            (context.group.description.as_str())
                        </textarea>
                    </div>
                    <button class="portal-button portal-button-primary" type="submit">
                        "Save Settings"
                    </button>
                </form>
            )
            if let Some(users) = transfer_candidates {
                let has_transfer_candidates = !users.is_empty();
                let transfer_confirmation = format!(
                "Transfer “{}” to the selected member? You will become an administrator.",
                context.group.name,
            );
                let delete_confirmation = format!(
                "Permanently delete “{}” and its synced items and files?",
                context.group.name,
                );
                section(
                    <h2 class="portal-section-heading">"Owner Options"</h2>
                    if has_transfer_candidates {
                    <section class="mt-6">
                        <h3 class="portal-section-heading">"Transfer Ownership"</h3>
                        <form
                            class="mt-4"
                            method="post"
                            action=(href!(
                            super::owner::transfer_owner,
                            GroupId(context.group.id),
                        ))
                        >
                            <label class="inline-block max-w-full">
                                <span class="block font-medium">"New Owner"</span>
                                <select
                                    class="mt-2 min-w-64 max-w-full border border-stone-300 bg-white px-3 py-2"
                                    name="username"
                                    required="required"
                                >
                                    for user in users {
                                        <option value=(user.username.as_str())>
                                            (user.username.as_str())
                                        </option>
                                    }
                                </select>
                            </label>
                            <div class="mt-4">
                                confirm_button(
                                    label: "Transfer",
                                    confirmation: transfer_confirmation.as_str(),
                                    class: "portal-button portal-button-secondary"
                                )
                            </div>
                        </form>
                    </section>
                    }
                    <section class=(if has_transfer_candidates {
                        "mt-8 border-t border-stone-200 pt-8"
                    } else {
                        "mt-6"
                    })>
                    <h3 class="portal-section-heading text-red-800">"Delete Group"</h3>
                    <p class="mt-2 text-stone-600">
                        "Permanently deletes the group and its synced library data."
                    </p>
                    <form
                        class="mt-4"
                        method="post"
                        action=(href!(
                        super::delete::delete_group,
                        GroupId(context.group.id),
                    ))
                    >
                        confirm_button(
                            label: "Delete Group",
                            confirmation: delete_confirmation.as_str(),
                            class: "border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50"
                        )
                    </form>
                    </section>
                )
            }
        </div>
    }
}

#[route(POST)]
async fn save_settings(cx: &Cx, Form(input): Form<GroupDetails>) -> Result<Response> {
    let context = managed_group_settings(cx).await?;
    let name = input.name.as_str();
    let description = input.description.trim();
    if description.chars().count() > 10_000 {
        return Err(bad_request("group description cannot exceed 10000 characters").into());
    }
    let mut group = context.group.clone();
    group.name = name.to_owned();
    group.description = description.to_owned();
    app_context::<Dataserver>(cx).update_group(&group).await?;
    see_other(
        href!(settings, GroupId(group.id))
            .query(SettingsQuery { saved: Some(true) })
            .resolve(cx),
    )
    .into_response(cx)
}
