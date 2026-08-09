use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found},
        header, page,
    },
    session,
    view::view,
};

use crate::{
    app::{AppState, request_confirmed, signed_in_user},
    store,
};

#[derive(Deserialize)]
struct RevokeSession {
    operation: String,
    handle: Option<String>,
}

#[page(POST)]
async fn revoke_session(cx: &Cx, Form(input): Form<RevokeSession>) -> Result {
    let user = signed_in_user(cx).await?;
    if input.operation == "all" {
        app_context::<AppState>(cx)
            .backend
            .revoke_all_browser_sessions(user.user_id)
            .await?;
        session::stop(cx).await?;
        let location = HeaderValue::from_static("/user/login");
        if request_confirmed(cx) {
            return view! { (StatusCode::NO_CONTENT) ((header::LOCATION, location)) };
        }
        return view! { (StatusCode::SEE_OTHER) ((header::LOCATION, location)) };
    }
    if input.operation != "one" {
        return Err(bad_request("unknown browser-session operation").into());
    }
    let handle = input
        .handle
        .as_deref()
        .ok_or_else(|| bad_request("a browser session is required"))?;
    let browser_session = app_context::<AppState>(cx)
        .backend
        .browser_sessions_for_user(user.user_id)
        .await?
        .into_iter()
        .find(|browser_session| browser_session.handle == handle)
        .ok_or_else(not_found)?;
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| store::session_handle(&*hash))
        .unwrap_or_default();
    if browser_session.handle == current_handle {
        return Err(forbidden().into());
    }
    if !app_context::<AppState>(cx)
        .backend
        .revoke_browser_session(user.user_id, &browser_session.handle)
        .await?
    {
        return Err(not_found().into());
    }
    if request_confirmed(cx) {
        return view! { (StatusCode::NO_CONTENT) };
    }
    view! { (StatusCode::SEE_OTHER) ((header::LOCATION, HeaderValue::from_static("/settings?notice=browser-session-revoked#sessions"))) }
}
