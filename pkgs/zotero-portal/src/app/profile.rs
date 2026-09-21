use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::not_found;
use topcoat::router::{href, page, path_param};
use topcoat::view::{View, component, view};

use crate::app::{authenticated_user, portal_page, section};
use crate::database::Database;
use crate::dataserver::Dataserver;
use crate::id::UserId;

topcoat::router::path_param!(pub(super) profile_username);

/// The official navbar's "My Profile" entry points to `/<username>`.
#[page("/{profile_username}")]
pub(super) async fn profile(cx: &Cx) -> Result<impl View> {
    let actor_id = authenticated_user(cx).await?;
    let username = path_param::<ProfileUsername>(cx);
    let user = app_context::<Database>(cx)
        .user_by_identifier(username)
        .await?
        .filter(|user| user.username == username)
        .ok_or_else(not_found)?;
    Ok(view! {
        portal_page(
            <div class="space-y-6">
                <header>
                    <h1 class="text-3xl font-semibold">(user.display_name())</h1>
                    <p class="mt-2 text-stone-600">
                        <span class="font-mono font-semibold">
                            (user.username.as_str())
                        </span>
                    </p>
                    <div class="mt-3 flex flex-wrap gap-4 text-sm">
                        if actor_id == user.user_id {
                            <a
                                class="portal-link"
                                href=(href!(
                                    crate::app::_web_library::personal_library_root,
                                    crate::app::_web_library::UserSlug(user.username.clone()),
                                ))
                            >
                                "Library"
                            </a>
                        }
                    </div>
                </header>
                profile_groups(actor_id: actor_id, profile_user_id: user.user_id)
            </div>
        )
    })
}

#[component]
async fn profile_groups(cx: &Cx, actor_id: UserId, profile_user_id: UserId) -> Result<impl View> {
    let mut groups = app_context::<Dataserver>(cx)
        .groups_for_user(actor_id)
        .await?;
    groups.retain(|group| group.role(profile_user_id).is_some());
    groups.sort_by_key(|left| left.name.to_lowercase());
    Ok(view! {
        section(
            <h2 class="text-xl font-semibold">"Groups"</h2>
            if groups.is_empty() {
                <p class="mt-3 text-stone-600">"No shared groups."</p>
            } else {
                <ul class="mt-4 divide-y divide-stone-200">
                    for group in groups {
                        <li
                            class="flex flex-wrap items-center justify-between gap-3 py-3"
                        >
                            <a
                                class="portal-link font-medium"
                                href=(href!(
                                    crate::app::groups::group_overview,
                                    crate::app::groups::GroupId(group.id),
                                    crate::app::groups::GroupSlug(group.slug()),
                                ))
                            >
                                (group.name.as_str())
                            </a>
                            <span class="text-sm text-stone-600">
                                (group.user_ids().len())
                                (if group.user_ids().len() == 1 {
                                    " member"
                                } else {
                                    " members"
                                })
                            </span>
                        </li>
                    }
                </ul>
            }
        )
    })
}
