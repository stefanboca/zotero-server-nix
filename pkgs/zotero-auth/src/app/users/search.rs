use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{StatusCode, query_params, route},
    view::{View, component, view},
};

use crate::{
    app::{AppState, authenticated_user, username_label},
    store::UserSummary,
};

#[query_params(error = bad_request)]
struct SearchQuery {
    q: String,
}

#[route(GET)]
async fn search(cx: &Cx) -> Result<View> {
    let parameters = query_params::<SearchQuery>(cx)?;
    let query = parameters.q.trim();
    let user_id = authenticated_user(cx).await?;
    if query.chars().count() < 3 || query.chars().count() > 128 {
        return view! { cx => search_results(query: query, candidates: &[]) };
    }
    let app = app_context::<AppState>(cx);
    if !app.backend.allow_user_search(user_id).await? {
        return view! { cx =>
            (StatusCode::TOO_MANY_REQUESTS)
            <div class="p-5 text-stone-600 dark:text-stone-400" role="status">
                "Too many searches. Wait a minute, or enter an exact username or email and select Add."
            </div>
        };
    }
    let candidates = app.backend.search_users(query, &[], 10).await?;
    view! { cx => search_results(query: query, candidates: &candidates) }
}

#[component]
async fn search_results(query: &str, candidates: &[UserSummary]) -> Result {
    view! {
        <div class="p-3">
            <p class="px-2 pb-2 text-sm text-stone-600 dark:text-stone-400">
                "Results for “" (query) "”"
            </p>
            if candidates.is_empty() {
                <p class="border-t border-stone-200 px-2 py-4 text-stone-600 dark:border-stone-800 dark:text-stone-400">
                    "No matching users found. You can still enter an exact username or email."
                </p>
            } else {
                <ul class="divide-y divide-stone-200 border-t border-stone-200 dark:divide-stone-800 dark:border-stone-800">
                    for candidate in candidates {
                        let option_id = format!("member-option-{}", candidate.username);
                        <li>
                            <button class="w-full px-2 py-3 text-left hover:bg-stone-100 focus:bg-stone-100 aria-selected:bg-stone-100 dark:hover:bg-stone-900 dark:focus:bg-stone-900 dark:aria-selected:bg-stone-900" id=(option_id) type="button" role="option" aria-selected="false" data-member-option="" data-username=(candidate.username.as_str())>
                                <p class="font-medium">(candidate.display_name())</p>
                                <p class="break-words text-sm text-stone-600 dark:text-stone-400">
                                    username_label(username: candidate.username.as_str()) " · " (candidate.email.as_str())
                                </p>
                            </button>
                        </li>
                    }
                </ul>
            }
        </div>
    }
}
