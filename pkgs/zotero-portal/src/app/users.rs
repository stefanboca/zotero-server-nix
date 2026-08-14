use topcoat::{
    Result,
    context::{Cx, app_context},
    runtime::shard,
    view::view,
};

use crate::{
    app::{authenticated_user, username_label},
    database::Database,
};

#[shard]
pub(crate) async fn member_search_shard(cx: &Cx, query: String) -> Result {
    let query = query.trim();
    authenticated_user(cx).await?;
    if query.chars().count() < 3 || query.chars().count() > 128 {
        return view! {};
    }
    let database = app_context::<Database>(cx);
    let candidates = database.search_users(query, 10).await?;
    view! {
        <div class="p-3">
            if candidates.is_empty() {
                <p class="px-2 py-4 text-stone-600 dark:text-stone-400">
                    "No matching users found. You can still enter an exact username or email."
                </p>
            } else {
                <ul class="divide-y divide-stone-200 dark:divide-stone-800">
                    for candidate in candidates {
                        <li>
                            <button
                                class="w-full px-2 py-3 text-left hover:bg-stone-100 focus:bg-stone-100 aria-selected:bg-stone-100 dark:hover:bg-stone-900 dark:focus:bg-stone-900 dark:aria-selected:bg-stone-900"
                                id=(format!(
                                    "member-option-{}", candidate.username
                                ))
                                type="button"
                                role="option"
                                aria-selected="false"
                                data-member-option=""
                                data-username=(candidate.username.as_str())
                            >
                                <p class="font-medium">(candidate.display_name())</p>
                                <p
                                    class="break-words text-sm text-stone-600 dark:text-stone-400"
                                >
                                    username_label(username: candidate.username.as_str())
                                    " · "
                                    (candidate.email.as_str())
                                </p>
                            </button>
                        </li>
                    }
                </ul>
            }
        </div>
    }
}
