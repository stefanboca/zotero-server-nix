use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::bad_request, page, query_params},
    view::view,
};

use crate::app::{AppState, is_topcoat_dev, valid_session_token};

#[query_params(error = bad_request)]
struct LoginQuery {
    #[serde(rename = "session")]
    dataserver_session: String,
}

#[page]
async fn login(cx: &Cx) -> Result {
    let action = if is_topcoat_dev() {
        "/auth/oidc/start?session=preview".to_owned()
    } else {
        let query = query_params::<LoginQuery>(cx)?;
        if !valid_session_token(&query.dataserver_session) {
            return Err(bad_request("invalid Zotero login-session token").into());
        }

        let app = app_context::<AppState>(cx);
        let info = app
            .dataserver
            .login_session_info(&query.dataserver_session)
            .await?;
        if info.status != "pending" {
            return Err(bad_request("Zotero login session is no longer pending").into());
        }
        format!("/auth/oidc/start?session={}", query.dataserver_session)
    };

    view! {
        <main
            class="w-full max-w-md rounded-xl border border-stone-200 bg-white p-8 shadow-sm dark:border-stone-800 dark:bg-stone-900"
        >
            <h1 class="text-2xl font-semibold">"Sign in to Zotero"</h1>
            <p class="mt-3 leading-7 text-stone-700 dark:text-stone-300">
                "Use your OpenID Connect account to continue."
            </p>
            <form class="mt-6" method="post" action=(action)>
                <button
                    class="w-full cursor-pointer rounded-lg bg-red-800 px-4 py-3 font-semibold text-white hover:bg-red-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-red-800 dark:bg-red-700 dark:hover:bg-red-600"
                    type="submit"
                >
                    "Continue with OpenID Connect"
                </button>
            </form>
        </main>
    }
}
