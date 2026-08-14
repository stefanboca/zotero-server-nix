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

use crate::{
    app::{BareLayout, FLOW_LIFETIME, current_user, normalized_return_path},
    backend::{Backend, OidcFlow},
    epoch_seconds,
};

#[query_params(error = bad_request)]
struct LoginQuery {
    session: Option<String>,
    redirect_to: Option<String>,
}

fn redirect_to(query: &LoginQuery) -> Result<String> {
    let value = query.redirect_to.as_deref().unwrap_or("/");
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
    let params = if let Some(dataserver_session) = query.session.as_deref() {
        let backend = app_context::<Backend>(cx);
        let info = backend.login_session_info(dataserver_session).await?;
        if info.status != "pending" {
            return Err(bad_request("Zotero login session is no longer pending").into());
        }
        if let Some(browser_session) = session::refresh(cx).await? {
            let expires = browser_session
                .expires_at
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let authenticated = backend
                .refresh_browser_session(&*browser_session.token_hash, expires)
                .await?;
            if authenticated.is_some() {
                let url = begin_oidc_flow(cx, Some(dataserver_session), "/", Some(browser_session))
                    .await?;
                return Err(redirect(url.as_str()).into());
            }
        }
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("session", dataserver_session)
            .finish()
    } else {
        let redirect_to = redirect_to(query)?;
        if current_user(cx).await?.is_some() {
            return Err(redirect(&redirect_to).into());
        }
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("redirect_to", &redirect_to)
            .finish()
    };
    view! {
        <main
            class="mx-auto flex min-h-screen w-full max-w-lg items-center px-4 py-12 sm:px-6"
        >
            <section class="w-full">
                <h1 class="text-2xl font-semibold">"Sign in to Zotero"</h1>
                <form class="mt-5" method="post" action=(format!("/user/login?{params}"))>
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
    let (redirect_to, browser_session) = if let Some(dataserver_session) = query.session.as_deref()
    {
        let backend = app_context::<Backend>(cx);
        let info = backend.login_session_info(dataserver_session).await?;
        if info.status != "pending" {
            return Err(bad_request("Zotero login session is no longer pending").into());
        }
        ("/".to_owned(), None)
    } else {
        let redirect_to = redirect_to(query)?;
        if current_user(cx).await?.is_some() {
            return see_other(&redirect_to).into_response(cx);
        }
        (redirect_to, session::refresh(cx).await?)
    };

    let url = begin_oidc_flow(cx, query.session.as_deref(), &redirect_to, browser_session).await?;
    see_other(url.as_str()).into_response(cx)
}

async fn begin_oidc_flow(
    cx: &Cx,
    dataserver_session: Option<&str>,
    return_path: &str,
    browser_session: Option<session::Session>,
) -> Result<String> {
    let backend = app_context::<Backend>(cx);
    if let Some(token_hash) = session::token_hash(cx).await?
        && !backend.clear_oidc_flow(&*token_hash).await?
    {
        return Err(bad_request("the current OIDC login is already being completed").into());
    }

    let authorization = backend.begin_oidc_authorization()?;
    let new_browser_session = browser_session.is_none();
    let browser_session = match browser_session {
        Some(browser_session) => browser_session,
        None => session::start(cx).await?,
    };
    let now = epoch_seconds()?;
    backend.delete_expired_oidc_flows(now).await?;
    let expires = now
        .checked_add(FLOW_LIFETIME.as_secs())
        .ok_or_else(|| anyhow::anyhow!("OIDC flow expiry is out of range"))?;
    let flow = OidcFlow {
        state: authorization.state,
        nonce: authorization.nonce,
        pkce_verifier: authorization.pkce_verifier,
        session_token: dataserver_session.map(str::to_owned),
        return_path: return_path.to_owned(),
        expires,
    };
    let inserted = backend
        .create_oidc_flow(&*browser_session.token_hash, &flow)
        .await;
    let inserted = match inserted {
        Ok(inserted) => inserted,
        Err(error) => {
            if new_browser_session {
                session::stop(cx).await?;
            }
            return Err(error.into());
        }
    };
    if !inserted {
        if new_browser_session {
            session::stop(cx).await?;
        }
        return Err(bad_request(
            "an OIDC login is already being completed for this Zotero session",
        )
        .into());
    }
    Ok(authorization.url)
}
