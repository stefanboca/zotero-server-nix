mod group_id;
mod r#new;

pub(in crate::app) use group_id::{group_id as group_id_param, group_overview, group_slug_param};
use serde::Deserialize;
use topcoat::context::{Cx, context};
use topcoat::router::{href, page};
use topcoat::runtime::{shard, signal};
use topcoat::view::{View, ViewExt as _, component, view};
use topcoat::{Error, Result};

use crate::app::{portal_page, section, signed_in_user};
use crate::database::{Database, GroupInvitation};
use crate::dataserver::{Dataserver, Group, GroupRole, group_slug};
use crate::id::UserId;

const RESERVED_GROUP_SLUGS: &[&str] = &[
    "delete",
    "invitations",
    "invite",
    "members",
    "owner",
    "settings",
];
const GROUPS_PER_PAGE: usize = 25;

#[derive(Deserialize)]
#[serde(try_from = "String")]
pub(super) struct GroupName(String);

impl GroupName {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GroupName {
    type Error = anyhow::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        let value = value.trim();
        if value.is_empty() {
            anyhow::bail!("group name cannot be empty");
        }
        if value.chars().count() > 255 {
            anyhow::bail!("group name cannot exceed 255 characters");
        }
        if value.chars().any(char::is_control) {
            anyhow::bail!("group name cannot contain control characters");
        }
        let slug = group_slug(value);
        if slug.is_empty() || matches!(slug.as_str(), "." | "..") {
            anyhow::bail!("group name must contain letters or another URL-safe character");
        }
        if RESERVED_GROUP_SLUGS.contains(&slug.as_str()) {
            anyhow::bail!("group name conflicts with a reserved group route");
        }
        Ok(Self(value.to_owned()))
    }
}

#[page]
pub(super) async fn list_groups(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    Ok(view! {
        portal_page(
            <div class="space-y-6">
                incoming_invitations(user_id: user.user_id)
                section(
                    <div>
                        <h1 class="text-3xl font-semibold">"Zotero Groups"</h1>
                        <div class="mt-3 flex flex-wrap gap-x-4 gap-y-2 text-sm">
                            <a class="portal-link" href=(href!(r#new::new_group))>
                                "Create a New Group"
                            </a>
                        </div>
                    </div>
                    groups_list()
                )
            </div>
        )
    })
}

#[component]
async fn groups_list(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    let dataserver = context::<Dataserver>(cx);
    let groups = dataserver
        .groups_for_user(user.user_id)
        .await
        .map_err(Error::from_anyhow)?;
    let total = groups.len();
    Ok(view! {
        if !groups.is_empty() {
            <div class="mt-5 border-t border-stone-300">
                #[key(group.id)]
                for group in groups.iter().take(GROUPS_PER_PAGE) {
                    group_card(group: group, user_id: user.user_id)
                }
                if total > GROUPS_PER_PAGE {
                    more_groups(offset: GROUPS_PER_PAGE)
                }
            </div>
        }
    })
}

#[component]
async fn more_groups(cx: &Cx, offset: usize) -> Result<impl View> {
    let load = signal(cx, || false);
    let offset = signal(cx, || offset.to_string());

    Ok(view! {
        <div class="pt-5" :hidden=$(load.get())>
            <button
                class="portal-link"
                type="button"
                @click=$(|_event: topcoat::runtime::Event| load.set(true))
            >
                "More"
            </button>
        </div>
        groups_page(offset: $(offset.get()), load: $(load.get()))
    }
    .boxed())
}

#[shard]
async fn groups_page(cx: &Cx, offset: String, load: bool) -> Result<impl View> {
    if !load {
        return Ok(view! {}.boxed());
    }
    let user = signed_in_user(cx).await?;
    let offset = offset.parse::<usize>()?;
    let groups = context::<Dataserver>(cx)
        .groups_for_user(user.user_id)
        .await
        .map_err(Error::from_anyhow)?;
    let next_offset = offset.saturating_add(GROUPS_PER_PAGE);
    Ok(view! {
        #[key(group.id)]
        for group in groups.iter().skip(offset).take(GROUPS_PER_PAGE) {
            group_card(group: group, user_id: user.user_id)
        }
        if next_offset < groups.len() {
            more_groups(offset: next_offset)
        }
    }
    .boxed())
}

#[component]
async fn group_card(cx: &Cx, group: &Group, user_id: UserId) -> Result<impl View> {
    let role = group.role(user_id).unwrap_or(GroupRole::Member);
    let member_count = group.user_ids().len();
    let slug = group.slug();
    Ok(view! {
        <article class="border-b border-stone-300 py-5">
            <h3 class="text-lg font-semibold">
                <a
                    class="portal-link"
                    href=(href!(
                        group_id::group_overview,
                        group_id::group_id: group.id,
                        group_id::group_slug_param: slug.clone(),
                    ))
                >
                    (group.name.as_str())
                </a>
            </h3>
            <a
                class="portal-link mt-1 inline-block text-sm"
                href=(href!(
                    crate::app::_web_library::group_library_path,
                    crate::app::_web_library::library_group_id: group.id,
                    crate::app::_web_library::library_group_slug: slug,
                    crate::app::_web_library::library_path: ["items"],
                ))
            >
                "Group Library"
            </a>
            if role.can_manage() {
                <div class="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
                    <a
                        class="portal-link"
                        href=(href!(
                            group_id::group_settings,
                            group_id::group_id: group.id,
                        ))
                    >
                        "Manage Profile"
                    </a>
                    <span aria-hidden="true">"·"</span>
                    <a
                        class="portal-link"
                        href=(href!(
                            group_id::members_settings,
                            group_id::group_id: group.id,
                        ))
                    >
                        "Manage Members"
                    </a>
                    <span aria-hidden="true">"·"</span>
                    <a
                        class="portal-link"
                        href=(href!(
                            group_id::library_settings,
                            group_id::group_id: group.id,
                        ))
                    >
                        "Manage Library"
                    </a>
                </div>
            }
            <dl class="mt-4 grid gap-x-6 gap-y-2 text-sm sm:grid-cols-[8rem_1fr]">
                <dt class="text-stone-600">"Members"</dt>
                <dd>(member_count)</dd>
                if !group.description.trim().is_empty() {
                    <dt class="text-stone-600">"Description"</dt>
                    <dd class="whitespace-pre-wrap">(group.description.as_str())</dd>
                }
            </dl>
        </article>
    })
}

#[component]
async fn incoming_invitations(cx: &Cx, user_id: UserId) -> Result<impl View> {
    let invitations = context::<Database>(cx)
        .group_invitations_for_user(user_id)
        .await
        .map_err(Error::from_anyhow)?;
    if invitations.is_empty() {
        return Ok(view! {}.boxed());
    }
    Ok(view! {
        section(
            <h2 class="portal-section-heading">"Group Invitations"</h2>
            <div class="mt-4 divide-y divide-stone-200">
                #[key(invitation.group_id)]
                for invitation in invitations {
                    incoming_invitation(invitation: invitation)
                }
            </div>
        )
    }
    .boxed())
}

#[component]
async fn incoming_invitation(cx: &Cx, invitation: GroupInvitation) -> Result<impl View> {
    let Some(group) = context::<Dataserver>(cx)
        .group(invitation.group_id)
        .await
        .map_err(Error::from_anyhow)?
    else {
        return Ok(view! {}.boxed());
    };
    Ok(view! {
        <div
            class="flex flex-wrap items-center justify-between gap-4 py-4 first:pt-0 last:pb-0"
        >
            <div>
                <a
                    class="portal-link font-semibold"
                    href=(href!(
                        group_id::group_overview,
                        group_id::group_id: group.id,
                        group_id::group_slug_param: group.slug(),
                    ))
                >
                    (group.name.as_str())
                </a>
                <p class="mt-1 text-sm text-stone-600">
                    "You have been invited to join this group."
                </p>
            </div>
            <div class="flex gap-2">
                <form
                    method="post"
                    action=(href!(
                        group_id::accept_invitation,
                        group_id::group_id: group.id,
                    ))
                >
                    <button
                        class="portal-button portal-button-primary text-sm"
                        type="submit"
                    >
                        "Join"
                    </button>
                </form>
                <form
                    method="post"
                    action=(href!(
                        group_id::ignore_invitation,
                        group_id::group_id: group.id,
                    ))
                >
                    <button
                        class="portal-button portal-button-secondary text-sm"
                        type="submit"
                    >
                        "Ignore"
                    </button>
                </form>
            </div>
        </div>
    }
    .boxed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_names_cannot_shadow_group_routes() {
        for name in [
            "settings",
            "Invite",
            "invitations",
            "Members",
            "owner",
            "delete",
            "sett&ings",
            ".",
            "..",
            "&",
            "line\nbreak",
        ] {
            assert!(GroupName::try_from(name.to_owned()).is_err(), "{name}");
        }
        assert_eq!(
            GroupName::try_from(" Research Group ".to_owned())
                .expect("ordinary group name")
                .as_str(),
            "Research Group"
        );
    }
}
