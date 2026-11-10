use serde::Deserialize;
use topcoat::context::{Cx, context};
use topcoat::router::content::Form;
use topcoat::router::error::{not_found, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::ApiKeyValue;
use crate::app::signed_in_user;
use crate::dataserver::Dataserver;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeKey {
    key: ApiKeyValue,
}

#[route(POST)]
pub(in crate::app::settings) async fn revoke_key(
    cx: &Cx,
    Form(input): Form<RevokeKey>,
) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    if !context::<Dataserver>(cx)
        .delete_access_key(user.user_id, input.key.as_str())
        .await
        .map_err(Error::from_anyhow)?
    {
        return Err(not_found().into());
    }
    tracing::info!(user_id = %user.user_id, "API key revoked");
    see_other(
        href!(crate::app::settings::security::security)
            .fragment("applications")
            .resolve(cx),
    )
    .into_response(cx)
}
