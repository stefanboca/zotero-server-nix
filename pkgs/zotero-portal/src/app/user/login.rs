use topcoat::{
    Result,
    context::{Cx, CxBuilder, app_context},
    router::{
        Body, IntoResponse, Next, Response,
        error::{bad_request, redirect, see_other},
        layer, page, query_params, route,
    },
    session,
    view::view,
};

use crate::app::{AppState, BareLayout, current_user, normalized_return_path, valid_session_token};

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

#[layer]
async fn bare_login_layout(cx: &mut CxBuilder, body: Body, next: Next<'_>) -> Result<Response> {
    cx.insert(BareLayout);
    next.run(cx, body).await
}

#[page]
async fn login(cx: &Cx) -> Result {
    let query = query_params::<LoginQuery>(cx)?;
    let action = if let Some(dataserver_session) = query.session.as_deref() {
        if !valid_session_token(dataserver_session) {
            return Err(bad_request("invalid Zotero login-session token").into());
        }

        let app = app_context::<AppState>(cx);
        let info = app.backend.login_session_info(dataserver_session).await?;
        if info.status != "pending" {
            return Err(bad_request("Zotero login session is no longer pending").into());
        }
        if let Some(browser_session) = session::refresh(cx).await? {
            let expires = browser_session
                .expires_at
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let authenticated = app
                .backend
                .refresh_browser_session(&*browser_session.token_hash, expires)
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
        <main
            class="mx-auto flex min-h-screen w-full max-w-lg items-center px-4 py-12 sm:px-6"
        >
            <section class="w-full">
                <h1 class="text-2xl font-semibold">"Sign in to Zotero"</h1>
                <form class="mt-5" method="post" action=(action)>
                    <button
                        class="cursor-pointer bg-red-800 px-4 py-2 font-semibold text-white hover:bg-red-700 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-red-800 dark:bg-red-700 dark:hover:bg-red-600"
                        type="submit"
                    >
                        "Continue with OpenID Connect"
                    </button>
                </form>
            </section>
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
    if current_user(cx).await?.is_some() {
        return see_other(&return_path).into_response(cx);
    }

    let browser_session = session::refresh(cx).await?;
    let url = begin_oidc_flow(cx, None, &return_path, browser_session).await?;
    see_other(url.as_str()).into_response(cx)
}
