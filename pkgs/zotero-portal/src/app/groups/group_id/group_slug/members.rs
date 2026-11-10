use topcoat::context::{Cx, context};
use topcoat::router::{href, page};
use topcoat::view::{View, view};
use topcoat::{Error, Result};

use super::current_slug_group;
use crate::app::{portal_page, section};
use crate::database::Database;

#[page]
pub(super) async fn members(cx: &Cx) -> Result<impl View> {
    let (_, group, role) = current_slug_group(cx).await?;
    if role.is_none() {
        return Err(topcoat::router::error::not_found().into());
    }
    let users = context::<Database>(cx)
        .users_by_ids(&group.user_ids())
        .await
        .map_err(Error::from_anyhow)?;
    Ok(view! {
        portal_page(
            section(
                <header class="border-b border-stone-200 pb-6">
                    <h1 class="text-3xl font-semibold">"Group Members"</h1>
                    <p class="mt-2 text-stone-600">(group.name.as_str())</p>
                </header>
                <ul class="divide-y divide-stone-200 py-4">
                    for user in users {
                        let role = group.role(user.user_id);
                        <li
                            class="flex flex-wrap items-center justify-between gap-3 py-4"
                        >
                            <div>
                                <a
                                    class="portal-link font-medium"
                                    href=(href!(
                                        crate::app::profile::profile,
                                        crate::app::profile::profile_username: user.username.clone(),
                                    ))
                                >
                                    (user.display_name())
                                </a>
                                <p class="mt-1 font-mono text-sm text-stone-600">
                                    (user.username.as_str())
                                </p>
                            </div>
                            if let Some(role) = role {
                                <span class="text-sm text-stone-600">
                                    (role.display_name())
                                </span>
                            }
                        </li>
                    }
                </ul>
            )
        )
    })
}
