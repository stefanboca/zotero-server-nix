use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{href, page},
    view::view,
};

use crate::database::Database;

use super::current_slug_group;

#[page]
pub(super) async fn members(cx: &Cx) -> Result {
    let (_, group, _) = current_slug_group(cx).await?;
    let users = app_context::<Database>(cx)
        .users_by_ids(&group.user_ids())
        .await?;
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
            <header class="border-b border-stone-200 pb-6 dark:border-stone-800">
                <h1 class="text-3xl font-semibold">"Group Members"</h1>
                <p class="mt-2 text-stone-600 dark:text-stone-400">
                    (group.name.as_str())
                </p>
            </header>
            <ul class="divide-y divide-stone-200 py-4 dark:divide-stone-800">
                for user in users {
                    let role = group.role(user.user_id);
                    <li class="flex flex-wrap items-center justify-between gap-3 py-4">
                        <div>
                            <a
                                class="font-medium hover:text-red-800 dark:hover:text-red-400"
                                href=(href!(
                                    crate::app::profile::profile,
                                    crate::app::profile::ProfileUsername(user.username.clone()),
                                ))
                            >
                                (user.display_name())
                            </a>
                            <p
                                class="mt-1 font-mono text-sm text-stone-600 dark:text-stone-400"
                            >
                                (user.username.as_str())
                            </p>
                        </div>
                        if let Some(role) = role {
                            <span class="text-sm text-stone-600 dark:text-stone-400">
                                (role.display_name())
                            </span>
                        }
                    </li>
                }
            </ul>
        </main>
    }
}
