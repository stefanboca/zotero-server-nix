use topcoat::{
    Result,
    context::{Cx, CxBuilder, app_context},
    cookie::{Cookies as _, SameSite, cookie_store, private_cookies},
    router::{
        Body, IntoResponse, Next, Response,
        error::{bad_request, forbidden, redirect, see_other},
        layer, page, query_params, route,
    },
    session,
    view::view,
};

use crate::{
    app::{BareLayout, FLOW_LIFETIME, current_user, normalized_return_path},
    database::Database,
    dataserver::Dataserver,
    id::UserId,
    oidc::{Oidc, OidcFlow},
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
    let user_id = current_user(cx).await?;
    let params = if let Some(dataserver_session) = query.session.as_deref() {
        let database = app_context::<Database>(cx);
        let dataserver = app_context::<Dataserver>(cx);
        if let Some(user_id) = user_id {
            complete_desktop_login(database, dataserver, dataserver_session, user_id).await?;
            return Err(redirect("/user/login/done").into());
        }
        require_pending_desktop_login(dataserver, dataserver_session).await?;
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("session", dataserver_session)
            .finish()
    } else {
        let redirect_to = redirect_to(query)?;
        if user_id.is_some() {
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
                <form
                    class="mt-5"
                    method="post"
                    action=(format!("/user/login?{params}"))
                >
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
    let redirect_to = if let Some(dataserver_session) = query.session.as_deref() {
        let database = app_context::<Database>(cx);
        let dataserver = app_context::<Dataserver>(cx);
        if let Some(user_id) = current_user(cx).await? {
            complete_desktop_login(database, dataserver, dataserver_session, user_id).await?;
            return see_other("/user/login/done").into_response(cx);
        }
        require_pending_desktop_login(dataserver, dataserver_session).await?;
        "/".to_owned()
    } else {
        let redirect_to = redirect_to(query)?;
        if current_user(cx).await?.is_some() {
            return see_other(&redirect_to).into_response(cx);
        }
        redirect_to
    };

    let url = begin_oidc_flow(cx, query.session.as_deref(), &redirect_to).await?;
    see_other(url.as_str()).into_response(cx)
}

async fn begin_oidc_flow(
    cx: &Cx,
    dataserver_session: Option<&str>,
    return_path: &str,
) -> Result<String> {
    let authorization = app_context::<Oidc>(cx).begin_authorization()?;
    session::stop(cx).await?;
    let flow = OidcFlow {
        state: authorization.state,
        nonce: authorization.nonce,
        pkce_verifier: authorization.pkce_verifier,
        session_token: dataserver_session.map(str::to_owned),
        return_path: return_path.to_owned(),
        expires_at: jiff::Timestamp::now().checked_add(FLOW_LIFETIME)?,
    };
    cookie_store::<OidcFlow, _>(
        private_cookies(cx)
            .default_http_only(true)
            .default_same_site(SameSite::Lax)
            .default_path("/"),
        "oidc-flow",
    )
    .set(flow)
    .commit()?;
    Ok(authorization.url)
}

async fn require_pending_desktop_login(dataserver: &Dataserver, session_token: &str) -> Result<()> {
    if dataserver.login_session_info(session_token).await?.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    Ok(())
}

pub(crate) async fn complete_desktop_login(
    database: &Database,
    dataserver: &Dataserver,
    session_token: &str,
    user_id: UserId,
) -> Result<()> {
    // Dataserver's first-login shard setup is not atomic. Serialize completion
    // by user even when separate browser sessions log in concurrently.
    let transaction = database
        .lock_user_for_login_completion(user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user disappeared during login completion"))?;

    let info = dataserver.login_session_info(session_token).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if info.user_id.is_some_and(|existing| existing != user_id) {
        return Err(forbidden().into());
    }
    dataserver
        .complete_login_session(session_token, user_id, info.access)
        .await?;
    dataserver.set_unlimited_storage(user_id).await?;
    transaction.commit().await?;
    Ok(())
}

mod done;
