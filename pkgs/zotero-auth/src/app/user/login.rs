use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        IntoResponse, Response,
        error::{bad_request, redirect, see_other},
        page, query_params, route,
    },
    session,
    view::view,
};

use crate::{
    app::{AppState, current_user, is_topcoat_dev, normalized_return_path, valid_session_token},
    store,
};

use super::super::auth::begin_oidc_flow;

#[query_params(error = bad_request)]
struct LoginQuery {
    session: Option<String>,
    #[serde(rename = "continue")]
    return_path: Option<String>,
}

fn return_path(query: &LoginQuery) -> Result<String> {
    let value = query.return_path.as_deref().unwrap_or("/");
    normalized_return_path(value).ok_or_else(|| bad_request("invalid login return path").into())
}

#[page]
async fn login(cx: &Cx) -> Result {
    let query = query_params::<LoginQuery>(cx)?;
    let action = if is_topcoat_dev() {
        "/auth/oidc/start?session=preview".to_owned()
    } else if let Some(dataserver_session) = query.session.as_deref() {
        if !valid_session_token(dataserver_session) {
            return Err(bad_request("invalid Zotero login-session token").into());
        }

        let app = app_context::<AppState>(cx);
        let info = app
            .dataserver
            .login_session_info(dataserver_session)
            .await?;
        if info.status != "pending" {
            return Err(bad_request("Zotero login session is no longer pending").into());
        }
        if let Some(browser_session) = session::refresh(cx).await? {
            let expires = browser_session
                .expires_at
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let authenticated = store::refresh_browser_session(
                &app.database,
                &*browser_session.token_hash,
                expires,
            )
            .await?;
            if authenticated.is_some() {
                let url = begin_oidc_flow(cx, Some(dataserver_session), "/", Some(browser_session))
                    .await?;
                return Err(redirect(url.as_str()).into());
            }
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("session", dataserver_session)
            .finish();
        format!("/auth/oidc/start?{query}")
    } else {
        let return_path = return_path(query)?;
        if current_user(cx).await?.is_some() {
            return Err(redirect(&return_path).into());
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("continue", &return_path)
            .finish();
        format!("/user/login?{query}")
    };
    view! {
        <main class="mx-auto w-full max-w-4xl px-4 py-10 sm:px-6 sm:py-12">
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

#[route(POST)]
async fn start(cx: &Cx) -> Result<Response> {
    let query = query_params::<LoginQuery>(cx)?;
    if query.session.is_some() {
        return Err(bad_request("desktop login must use the OIDC start endpoint").into());
    }
    let return_path = return_path(query)?;
    if is_topcoat_dev() {
        return see_other(&return_path).into_response(cx);
    }
    if current_user(cx).await?.is_some() {
        return see_other(&return_path).into_response(cx);
    }

    let browser_session = session::refresh(cx).await?;
    let url = begin_oidc_flow(cx, None, &return_path, browser_session).await?;
    see_other(url.as_str()).into_response(cx)
}
