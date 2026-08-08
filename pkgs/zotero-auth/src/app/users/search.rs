use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{query_params, route},
    view::{View, component, view},
};

use crate::{
    app::{AppState, authenticated_user, is_topcoat_dev, username_label},
    store::{self, UserSummary},
};

#[query_params(error = topcoat::router::error::bad_request)]
struct SearchQuery {
    q: String,
}

#[route(GET)]
async fn search(cx: &Cx) -> Result<View> {
    let parameters = query_params::<SearchQuery>(cx)?;
    let query = parameters.q.trim();
    authenticated_user(cx).await?;
    if is_topcoat_dev() {
        let candidates = vec![UserSummary {
            user_id: 4,
            username: "researcher".to_owned(),
            email: "researcher@example.com".to_owned(),
            real_name: Some("New Researcher".to_owned()),
        }];
        return view! { cx => search_results(query: query, candidates: &candidates) };
    }
    let app = app_context::<AppState>(cx);
    let candidates = store::search_oidc_users(&app.database, query, &[], 20).await?;
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
