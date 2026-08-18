mod library;
mod members;

pub(in crate::app::groups) use library::library as library_settings;
pub(in crate::app::groups) use members::members as members_settings;

use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context, request_context},
    router::{
        Body, Next,
        content::Form,
        error::{bad_request, forbidden, see_other},
        href, layer, layout,
        request::uri,
        response::{IntoResponse, Response},
        route,
    },
    view::{PromotedStr, view},
};

use crate::{
    app::{confirm_button, groups::GroupName},
    database::Database,
    dataserver::{Dataserver, Group, GroupRole},
    id::UserId,
};

use super::{GroupId, current_group};

#[derive(Clone)]
pub(super) struct GroupSettingsContext {
    pub(super) group: Group,
    pub(super) actor_id: UserId,
    pub(super) actor_role: GroupRole,
}

pub(super) fn settings_context(cx: &Cx) -> &GroupSettingsContext {
    request_context::<GroupSettingsContext>(cx)
}

#[layer]
async fn managed_group_settings(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    if !actor_role.can_manage() {
        return Err(forbidden().into());
    }
    let cx = cx.with(GroupSettingsContext {
        group,
        actor_id,
        actor_role,
    });
    next.run(&cx, body).await
}

#[layout]
async fn group_settings_layout(cx: &Cx, slot: Result) -> Result {
    let context = settings_context(cx);
    let path = uri(cx).path();
    let group_id = context.group.id;
    let navigation = [
        (
            href!(settings, GroupId(group_id)).resolve(cx),
            PromotedStr(&"General"),
        ),
        (
            href!(library::library, GroupId(group_id)).resolve(cx),
            PromotedStr(&"Library"),
        ),
        (
            href!(members::members, GroupId(group_id)).resolve(cx),
            PromotedStr(&"Members"),
        ),
    ];
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <div class="grid gap-8 md:grid-cols-[12rem_minmax(0,1fr)]">
                <nav aria-label="Group settings sections">
                    <h1 class="break-words text-lg font-semibold">
                        (context.group.name.as_str())
                    </h1>
                    <ul
                        class="mt-3 border-y border-stone-200 py-2 dark:border-stone-800"
                    >
                        for (href, label) in navigation {
                            let active = path == href;
                            <li>
                                <a
                                    class=(if active {
                                        "block border-l-2 border-red-800 bg-red-50 px-3 py-2 font-semibold text-red-800 dark:border-red-400 dark:bg-red-950 dark:text-red-400"
                                    } else {
                                        "block border-l-2 border-transparent px-3 py-2 hover:bg-stone-100 dark:hover:bg-stone-900"
                                    })
                                    href=(href)
                                    aria-current=(active.then_some("page"))
                                >
                                    (label)
                                </a>
                            </li>
                        }
                    </ul>
                </nav>
                <div class="min-w-0">(slot?)</div>
            </div>
        </main>
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupDetails {
    name: GroupName,
    description: String,
    url: String,
}

#[topcoat::router::page]
pub(in crate::app::groups) async fn settings(cx: &Cx) -> Result {
    let context = settings_context(cx);
    let owner_users = if context.actor_role == GroupRole::Owner {
        Some(
            app_context::<Database>(cx)
                .users_by_ids(&context.group.user_ids())
                .await?,
        )
    } else {
        None
    };
    view! {
        <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
            <h1 class="text-3xl font-semibold">"Group Settings"</h1>
            <p class="mt-2 text-stone-600 dark:text-stone-400">
                "Update the group’s name and profile."
            </p>
        </header>
        <form
            class="space-y-6 py-8"
            method="post"
            action=(href!(settings, GroupId(context.group.id)))
        >
            <div>
                <label class="block font-medium" for="group-name">"Name"</label>
                <input
                    class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
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
                    class="mt-2 min-h-32 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                    id="group-description"
                    name="description"
                    maxlength="10000"
                >
                    (context.group.description.as_str())
                </textarea>
            </div>
            <div>
                <label class="block font-medium" for="group-url">"Group URL"</label>
                <input
                    class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                    id="group-url"
                    name="url"
                    type="url"
                    value=(context.group.url.as_str())
                    maxlength="2048"
                >
            </div>
            <button
                class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700"
                type="submit"
            >
                "Save Settings"
            </button>
        </form>

        if let Some(users) = owner_users {
            let transfer_confirmation = format!(
                "Transfer “{}” to the selected member? You will become an administrator.",
                context.group.name,
            );
            let delete_confirmation = format!(
                "Permanently delete “{}” and its synced items and files?",
                context.group.name,
            );
            <section class="border-t border-stone-200 py-8 dark:border-stone-800">
                <h2 class="text-xl font-semibold">"Transfer Ownership"</h2>
                <form
                    class="mt-4 flex flex-wrap items-end gap-3"
                    method="post"
                    action=(href!(
                        super::owner::transfer_owner,
                        GroupId(context.group.id),
                    ))
                >
                    <label class="min-w-0 flex-1">
                        <span class="block font-medium">"New Owner"</span>
                        <select
                            class="mt-2 w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                            name="username"
                            required="required"
                        >
                            <option value="">"Select a member"</option>
                            for user in &users {
                                if user.user_id != context.group.owner {
                                    <option value=(user.username.as_str())>
                                        (user.display_name())
                                        " ("
                                        (user.username.as_str())
                                        ")"
                                    </option>
                                }
                            }
                        </select>
                    </label>
                    confirm_button(
                        label: "Transfer",
                        confirmation: transfer_confirmation.as_str(),
                        class: "border border-stone-300 px-4 py-2 font-medium hover:bg-stone-100 dark:border-stone-700 dark:hover:bg-stone-900"
                    )
                </form>
            </section>
            <section class="border-t border-stone-200 py-8 dark:border-stone-800">
                <h2 class="text-xl font-semibold text-red-800 dark:text-red-400">
                    "Delete Group"
                </h2>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
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
                        class: "border border-red-800 px-4 py-2 font-medium text-red-800 hover:bg-red-50 dark:border-red-400 dark:text-red-400 dark:hover:bg-red-950"
                    )
                </form>
            </section>
        }
    }
}

#[route(POST)]
async fn save_settings(cx: &Cx, Form(input): Form<GroupDetails>) -> Result<Response> {
    let context = settings_context(cx);
    let name = input.name.as_str();
    let description = input.description.trim();
    if description.chars().count() > 10_000 {
        return Err(bad_request("group description cannot exceed 10000 characters").into());
    }
    let url = input.url.trim();
    if url.len() > 2048 {
        return Err(bad_request("group URL cannot exceed 2048 bytes").into());
    }
    if !url.is_empty() {
        let parsed = url::Url::parse(url).map_err(|_| bad_request("group URL is invalid"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(bad_request("group URL must use HTTP or HTTPS").into());
        }
    }
    let mut group = context.group.clone();
    group.name = name.to_owned();
    group.description = description.to_owned();
    group.url = url.to_owned();
    app_context::<Dataserver>(cx).update_group(&group).await?;
    see_other(href!(settings, GroupId(group.id)).resolve(cx)).into_response(cx)
}
