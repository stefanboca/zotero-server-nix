use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        IntoResponse, Response,
        error::{bad_request, see_other},
        query_params, route,
    },
    session,
};

use crate::app::{AppState, FLOW_LIFETIME, epoch_seconds, valid_session_token};

#[query_params(error = bad_request)]
struct StartQuery {
    #[serde(rename = "session")]
    dataserver_session: Option<String>,
}

#[route(POST)]
async fn start(cx: &Cx) -> Result<Response> {
    let query = query_params::<StartQuery>(cx)?;
    let dataserver_session = query
        .dataserver_session
        .as_deref()
        .ok_or_else(|| bad_request("missing Zotero login-session token"))?;
    if !valid_session_token(dataserver_session) {
        return Err(bad_request("invalid Zotero login-session token").into());
    }

    let app = app_context::<AppState>(cx);
    let info = app.backend.login_session_info(dataserver_session).await?;
    if info.status != "pending" {
        return Err(bad_request("Zotero login session is no longer pending").into());
    }
    let url = begin_oidc_flow(cx, Some(dataserver_session), "/", None).await?;

    // A Topcoat redirect error bypasses the cookie layer's response finalizer.
    // Return a successful redirect so session::start can emit its cookie.
    see_other(url.as_str()).into_response(cx)
}

pub(crate) async fn begin_oidc_flow(
    cx: &Cx,
    dataserver_session: Option<&str>,
    return_path: &str,
    browser_session: Option<session::Session>,
) -> Result<String> {
    let app = app_context::<AppState>(cx);
    if let Some(token_hash) = session::token_hash(cx).await?
        && !app.backend.clear_oidc_flow(&*token_hash).await?
    {
        return Err(bad_request("the current OIDC login is already being completed").into());
    }

    let authorization = app.backend.begin_oidc_authorization().await?;
    let new_browser_session = browser_session.is_none();
    let browser_session = match browser_session {
        Some(browser_session) => browser_session,
        None => session::start(cx).await?,
    };
    let now = epoch_seconds()?;
    app.backend.delete_expired_oidc_flows(now).await?;
    let expires = now
        .checked_add(FLOW_LIFETIME.as_secs())
        .ok_or_else(|| anyhow::anyhow!("OIDC flow expiry is out of range"))?;
    let inserted = app
        .backend
        .create_oidc_flow(
            &*browser_session.token_hash,
            &authorization.state,
            &authorization.nonce,
            &authorization.pkce_verifier,
            dataserver_session,
            return_path,
            expires,
        )
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
