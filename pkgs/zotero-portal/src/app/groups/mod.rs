mod group_id;
mod r#new;

use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{href, page},
    runtime::shard,
    view::view,
};

use crate::{
    app::signed_in_user,
    dataserver::{Dataserver, GroupRole, group_slug},
};

const RESERVED_GROUP_SLUGS: &[&str] = &["delete", "members", "owner", "settings"];

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
pub(super) async fn list_groups(cx: &Cx) -> Result {
    signed_in_user(cx).await?;
    view! {
        signal groups_query = String::new();

        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <section class="py-8">
                <h1 class="text-2xl font-semibold">"Your Groups"</h1>
                <label class="mt-5 block max-w-xl">
                    <span class="sr-only">"Search your groups"</span>
                    <input
                        class="w-full border border-stone-300 bg-white px-3 py-2 dark:border-stone-700 dark:bg-stone-950"
                        type="search"
                        placeholder="Search your groups"
                        autocomplete="off"
                        :value=$(groups_query.get())
                        @input=$(|event: topcoat::runtime::Event| {
                            groups_query.set(event.target.value)
                        })
                    >
                </label>
                groups_shard(query: $(groups_query.get()))
                <div
                    class="mt-6 flex justify-end border-t border-stone-200 pt-6 dark:border-stone-800"
                >
                    <a
                        class="bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 dark:bg-red-700"
                        href=(href!(r#new::new_group))
                    >
                        "New Group"
                    </a>
                </div>
            </section>
        </main>
    }
}

#[shard]
async fn groups_shard(cx: &Cx, query: String) -> Result {
    let user = signed_in_user(cx).await?;
    let dataserver = app_context::<Dataserver>(cx);
    let mut groups = dataserver.groups_for_user(user.user_id).await?;
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        groups.retain(|group| group.name.to_lowercase().contains(&query));
    }
    groups.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    view! {
        if groups.is_empty() {
            <p class="mt-4 text-stone-600 dark:text-stone-400">
                (if query.is_empty() {
                    "You do not belong to any groups yet."
                } else {
                    "No groups match your search."
                })
            </p>
        } else {
            <ul class="mt-4 divide-y divide-stone-200 dark:divide-stone-800">
                for group in &groups {
                    let role = group.role(user.user_id).unwrap_or(GroupRole::Member);
                    let member_count = group.user_ids().len();
                    <li class="py-4">
                        let group_slug = group.slug();
                        let group_library = href!(
                            crate::app::_web_library::group_library_path,
                            crate::app::_web_library::LibraryGroupId(group.id),
                            crate::app::_web_library::LibraryGroupSlug(group_slug),
                            crate::app::_web_library::LibraryPath(["items"]),
                        );
                        <div
                            class="flex flex-wrap items-center justify-between gap-3 p-3 hover:bg-stone-100 dark:hover:bg-stone-800"
                        >
                            <div>
                                <a
                                    class="font-medium hover:text-red-800 dark:hover:text-red-400"
                                    href=(group_library)
                                >
                                    (group.name.as_str())
                                </a>
                                <p class="mt-1 text-sm text-stone-600 dark:text-stone-400">
                                    (role.user_summary())
                                    " · "
                                    (member_count)
                                    " "
                                    if member_count == 1 {
                                        "member"
                                    } else {
                                        "members"
                                    }
                                    " · Private"
                                </p>
                            </div>
                            if role.can_manage() {
                                <div class="flex flex-wrap gap-3 text-sm">
                                    <a
                                        class="hover:text-red-800 dark:hover:text-red-400"
                                        href=(href!(
                                            group_id::group_settings,
                                            group_id::GroupId(group.id),
                                        ))
                                    >
                                        "Settings"
                                    </a>
                                    <a
                                        class="hover:text-red-800 dark:hover:text-red-400"
                                        href=(href!(
                                            group_id::library_settings,
                                            group_id::GroupId(group.id),
                                        ))
                                    >
                                        "Library"
                                    </a>
                                    <a
                                        class="hover:text-red-800 dark:hover:text-red-400"
                                        href=(href!(
                                            group_id::members_settings,
                                            group_id::GroupId(group.id),
                                        ))
                                    >
                                        "Members"
                                    </a>
                                </div>
                            }
                        </div>
                    </li>
                }
            </ul>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_names_cannot_shadow_group_routes() {
        for name in [
            "settings",
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
