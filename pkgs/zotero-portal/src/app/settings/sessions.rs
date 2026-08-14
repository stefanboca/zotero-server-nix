use reqwest::header::{HeaderValue, LOCATION};
use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        IntoResponse as _, Response, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found, see_other},
        route,
    },
    session,
};

use crate::{
    app::{request_confirmed, signed_in_user},
    database::{self, Database},
    dataserver::Dataserver,
};

#[derive(Deserialize)]
struct RevokeSession {
    all: bool,
    handle: Option<String>,
}

#[route(POST)]
async fn revoke_session(cx: &Cx, Form(input): Form<RevokeSession>) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    let database = app_context::<Database>(cx);
    let dataserver = app_context::<Dataserver>(cx);
    if input.all {
        database
            .revoke_all_browser_sessions(dataserver, user.user_id)
            .await?;
        session::stop(cx).await?;
        if request_confirmed(cx) {
            return (
                StatusCode::NO_CONTENT,
                ([(LOCATION, HeaderValue::from_static("/user/login"))], ()),
            )
                .into_response(cx);
        }
        return see_other("/user/login").into_response(cx);
    } else {
        let handle = input
            .handle
            .as_deref()
            .ok_or_else(|| bad_request("a browser session is required"))?;
        // TODO: make this a new query
        let browser_session = database
            .browser_sessions_for_user(dataserver, user.user_id)
            .await?
            .into_iter()
            .find(|browser_session| browser_session.handle == handle)
            .ok_or_else(not_found)?;
        let current_handle = session::token_hash(cx)
            .await?
            .map(|hash| database::session_handle(&*hash))
            .unwrap_or_default();
        // TODO: should this be forbidden?
        if browser_session.handle == current_handle {
            return Err(forbidden().into());
        }
        if !database
            .revoke_browser_session(dataserver, user.user_id, &browser_session.handle)
            .await?
        {
            return Err(not_found().into());
        }
        if request_confirmed(cx) {
            return StatusCode::NO_CONTENT.into_response(cx);
        }
        see_other("/settings?notice=browser-session-revoked#sessions").into_response(cx)
    }
}
