use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{not_found, see_other},
        href,
        response::{IntoResponse as _, Response},
        route,
    },
    session,
};

use crate::{
    app::signed_in_user,
    database::{self, Database},
    dataserver::Dataserver,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeSession {
    handle: String,
}

#[route(POST)]
pub(super) async fn revoke_session(cx: &Cx, Form(input): Form<RevokeSession>) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    let database = app_context::<Database>(cx);
    let dataserver = app_context::<Dataserver>(cx);
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| database::session_handle(&*hash))
        .unwrap_or_default();
    if !database
        .revoke_browser_session(dataserver, user.user_id, &input.handle)
        .await?
    {
        return Err(not_found().into());
    }
    if input.handle == current_handle {
        session::stop(cx).await?;
        return see_other(href!(crate::app::user::login::login).resolve(cx)).into_response(cx);
    }
    see_other(
        href!(super::security::security)
            .fragment("sessions")
            .resolve(cx),
    )
    .into_response(cx)
}
