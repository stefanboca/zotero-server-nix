use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        IntoResponse as _, Response, StatusCode,
        content::Form,
        error::{bad_request, not_found, see_other},
        route,
    },
};

use crate::{
    app::{request_confirmed, signed_in_user},
    dataserver::Dataserver,
};

#[derive(Deserialize)]
struct RevokeClient {
    all: bool,
    handle: Option<String>,
}

#[route(POST)]
async fn revoke_client(cx: &Cx, Form(input): Form<RevokeClient>) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    let dataserver = app_context::<Dataserver>(cx);
    if input.all {
        dataserver
            .revoke_all_connected_clients(user.user_id)
            .await?;
        if request_confirmed(cx) {
            return StatusCode::NO_CONTENT.into_response(cx);
        }
        return see_other("/settings?notice=clients-revoked#apps").into_response(cx);
    } else {
        let handle = input
            .handle
            .as_deref()
            .ok_or_else(|| bad_request("a connected app is required"))?;
        let client = dataserver
            .connected_clients(user.user_id)
            .await?
            .into_iter()
            .find(|client| client.handle == handle)
            .ok_or_else(not_found)?;
        if !dataserver
            .revoke_connected_client(user.user_id, &client.handle)
            .await?
        {
            return Err(not_found().into());
        }
        if request_confirmed(cx) {
            return StatusCode::NO_CONTENT.into_response(cx);
        }
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("notice", "client-revoked")
            .append_pair("name", &client.name)
            .finish();
        see_other(&format!("/settings?{query}#apps")).into_response(cx)
    }
}
