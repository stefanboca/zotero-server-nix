use serde::Deserialize;
use topcoat::context::{Cx, context};
use topcoat::router::content::Form;
use topcoat::router::error::{SeeOther, not_found, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result, session};

use crate::app::signed_in_user;
use crate::database::{self, Database};
use crate::dataserver::Dataserver;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeSession {
    handle: String,
}

#[route(POST)]
pub(super) async fn revoke_session(cx: &Cx, Form(input): Form<RevokeSession>) -> Result<SeeOther> {
    let user = signed_in_user(cx).await?;
    let database = context::<Database>(cx);
    let dataserver = context::<Dataserver>(cx);
    let current_handle = session::token_hash(cx)
        .await?
        .map(|hash| database::session_handle(&*hash))
        .unwrap_or_default();
    if !database
        .revoke_browser_session(dataserver, user.user_id, &input.handle)
        .await
        .map_err(Error::from_anyhow)?
    {
        return Err(not_found().into());
    }
    tracing::info!(
        user_id = %user.user_id,
        current = input.handle == current_handle,
        "browser session revoked",
    );
    if input.handle == current_handle {
        session::stop(cx).await?;
        return Ok(see_other(href!(crate::app::user::login::login).resolve(cx)));
    }
    Ok(see_other(
        href!(super::security::security)
            .fragment("sessions")
            .resolve(cx),
    ))
}
