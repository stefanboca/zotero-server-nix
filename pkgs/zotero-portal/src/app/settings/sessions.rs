use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, forbidden, not_found},
        header, headers, page,
    },
    session,
    view::view,
};

use crate::{
    app::{AppState, signed_in_user},
    id::UserId,
    store::{self, BrowserSession},
};

#[derive(Deserialize)]
struct RevokeSession {
    operation: String,
    handle: Option<String>,
}

async fn browser_session(cx: &Cx, user_id: UserId, handle: &str) -> Result<BrowserSession> {
    app_context::<AppState>(cx)
        .backend
        .browser_sessions_for_user(user_id)
        .await?
        .into_iter()
        .find(|browser_session| browser_session.handle == handle)
        .ok_or_else(|| not_found().into())
}

async fn current_session_handle(cx: &Cx) -> Result<String> {
    Ok(session::token_hash(cx)
        .await?
        .map(|hash| store::session_handle(&*hash))
        .unwrap_or_default())
}

fn confirmed(cx: &Cx) -> bool {
    headers(cx)
        .get("x-zotero-confirmed")
        .is_some_and(|value| value == "true")
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
        if confirmed(cx) {
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
    let browser_session = browser_session(cx, user.user_id, handle).await?;
    if browser_session.handle == current_session_handle(cx).await? {
        return Err(forbidden().into());
    }
    if !app_context::<AppState>(cx)
        .backend
        .revoke_browser_session(user.user_id, &browser_session.handle)
        .await?
    {
        return Err(not_found().into());
    }
    if confirmed(cx) {
        return view! { (StatusCode::NO_CONTENT) };
    }
    view! { (StatusCode::SEE_OTHER) ((header::LOCATION, HeaderValue::from_static("/settings?notice=browser-session-revoked#sessions"))) }
}
