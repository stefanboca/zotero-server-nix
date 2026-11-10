pub(crate) mod done;

use topcoat::context::{Cx, context};
use topcoat::cookie::{Cookies as _, SameSite, cookie_store, private_cookies};
use topcoat::router::error::{SeeOther, bad_request, forbidden, redirect, see_other};
use topcoat::router::{href, page, query_params, route};
use topcoat::view::{View, view};
use topcoat::{Error, Result, session};

use crate::app::{FLOW_LIFETIME, current_user, normalized_return_path, portal_page, section};
use crate::database::Database;
use crate::dataserver::Dataserver;
use crate::id::UserId;
use crate::oidc::{Oidc, OidcFlow};

#[query_params(error = bad_request)]
#[derive(serde::Serialize)]
pub(crate) struct LoginQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) redirect_to: Option<String>,
}

fn redirect_to(query: &LoginQuery) -> Result<String> {
    let value = query.redirect_to.as_deref().unwrap_or("/");
    normalized_return_path(value).ok_or_else(|| bad_request("invalid login return path").into())
}

#[page]
pub(crate) async fn login(cx: &Cx) -> Result<impl View> {
    let query = query_params::<LoginQuery>(cx)?;
    let user_id = current_user(cx).await?;
    let form_query = if let Some(dataserver_session) = query.session.as_deref() {
        let database = context::<Database>(cx);
        let dataserver = context::<Dataserver>(cx);
        if let Some(user_id) = user_id {
            complete_desktop_login(database, dataserver, dataserver_session, user_id).await?;
            return Err(redirect(href!(done::done).resolve(cx)).into());
        }
        require_pending_desktop_login(dataserver, dataserver_session).await?;
        LoginQuery {
            session: Some(dataserver_session.to_owned()),
            redirect_to: None,
        }
    } else {
        let redirect_to = redirect_to(query)?;
        if user_id.is_some() {
            return Err(redirect(&redirect_to).into());
        }
        LoginQuery {
            session: None,
            redirect_to: Some(redirect_to),
        }
    };
    Ok(view! {
        portal_page(
            <div class="mx-auto max-w-lg text-center">
                section(
                    <h1 class="text-2xl font-semibold">"Sign in to Zotero"</h1>
                    <form
                        class="mt-5"
                        method="post"
                        action=(href!(start).query(&form_query))
                    >
                        <button
                            class="portal-button portal-button-primary"
                            type="submit"
                        >
                            "Continue with OpenID Connect"
                        </button>
                    </form>
                )
            </div>
        )
    })
}

#[route(POST)]
async fn start(cx: &Cx) -> Result<SeeOther> {
    let query = query_params::<LoginQuery>(cx)?;
    let redirect_to = if let Some(dataserver_session) = query.session.as_deref() {
        let database = context::<Database>(cx);
        let dataserver = context::<Dataserver>(cx);
        if let Some(user_id) = current_user(cx).await? {
            complete_desktop_login(database, dataserver, dataserver_session, user_id).await?;
            return Ok(see_other(href!(done::done).resolve(cx)));
        }
        require_pending_desktop_login(dataserver, dataserver_session).await?;
        "/".to_owned()
    } else {
        let redirect_to = redirect_to(query)?;
        if current_user(cx).await?.is_some() {
            return Ok(see_other(&redirect_to));
        }
        redirect_to
    };

    let url = begin_oidc_flow(cx, query.session.as_deref(), &redirect_to).await?;
    Ok(see_other(url.as_str()))
}

async fn begin_oidc_flow(
    cx: &Cx,
    dataserver_session: Option<&str>,
    return_path: &str,
) -> Result<String> {
    let authorization = context::<Oidc>(cx)
        .begin_authorization()
        .map_err(Error::from_anyhow)?;
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
    if dataserver
        .login_session_info(session_token)
        .await
        .map_err(Error::from_anyhow)?
        .status
        != "pending"
    {
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
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(|| anyhow::anyhow!("user disappeared during login completion"))
        .map_err(Error::from_anyhow)?;

    let info = dataserver
        .login_session_info(session_token)
        .await
        .map_err(Error::from_anyhow)?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    if info.user_id.is_some_and(|existing| existing != user_id) {
        return Err(forbidden().into());
    }
    dataserver
        .complete_login_session(session_token, user_id, info.access)
        .await
        .map_err(Error::from_anyhow)?;
    dataserver
        .set_unlimited_storage(user_id)
        .await
        .map_err(Error::from_anyhow)?;
    transaction.commit().await.map_err(Error::from_anyhow)
}
