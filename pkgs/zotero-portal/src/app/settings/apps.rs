use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        HeaderValue, StatusCode,
        content::Form,
        error::{bad_request, not_found},
        header, headers, page,
    },
    view::view,
};

use crate::{
    app::{AppState, signed_in_user},
    dataserver::ConnectedClient,
    id::UserId,
};

#[derive(Deserialize)]
struct RevokeClient {
    operation: String,
    handle: Option<String>,
}

async fn connected_client(cx: &Cx, user_id: UserId, handle: &str) -> Result<ConnectedClient> {
    app_context::<AppState>(cx)
        .backend
        .connected_clients(user_id)
        .await?
        .into_iter()
        .find(|client| client.handle == handle)
        .ok_or_else(|| not_found().into())
}

fn confirmed(cx: &Cx) -> bool {
    headers(cx)
        .get("x-zotero-confirmed")
        .is_some_and(|value| value == "true")
}

#[page(POST)]
async fn revoke_client(cx: &Cx, Form(input): Form<RevokeClient>) -> Result {
    let user = signed_in_user(cx).await?;
    if input.operation == "all" {
        app_context::<AppState>(cx)
            .backend
            .revoke_all_connected_clients(user.user_id)
            .await?;
        if confirmed(cx) {
            return view! { (StatusCode::NO_CONTENT) };
        }
        return view! { (StatusCode::SEE_OTHER) ((header::LOCATION, HeaderValue::from_static("/settings?notice=clients-revoked#apps"))) };
    }
    if input.operation != "one" {
        return Err(bad_request("unknown connected-app operation").into());
    }
    let handle = input
        .handle
        .as_deref()
        .ok_or_else(|| bad_request("a connected app is required"))?;
    let client = connected_client(cx, user.user_id, handle).await?;
    if !app_context::<AppState>(cx)
        .backend
        .revoke_connected_client(user.user_id, &client.handle)
        .await?
    {
        return Err(not_found().into());
    }
    if confirmed(cx) {
        return view! { (StatusCode::NO_CONTENT) };
    }
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("notice", "client-revoked")
        .append_pair("name", &client.name)
        .finish();
    let location = HeaderValue::try_from(format!("/settings?{query}#apps"))?;
    view! {
        (StatusCode::SEE_OTHER)
        ((header::LOCATION, location))
    }
}
