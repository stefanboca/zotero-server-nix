use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{not_found, see_other},
        href,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{app::signed_in_user, dataserver::Dataserver};

use super::ApiKeyValue;

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
    if !app_context::<Dataserver>(cx)
        .delete_access_key(user.user_id, input.key.as_str())
        .await?
    {
        return Err(not_found().into());
    }
    see_other(
        href!(crate::app::settings::security::security)
            .fragment("applications")
            .resolve(cx),
    )
    .into_response(cx)
}
