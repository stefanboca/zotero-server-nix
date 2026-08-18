use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::see_other,
        href,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::dataserver::Dataserver;

use super::super::settings_user;

#[route(POST)]
pub(super) async fn purge_personal_storage(cx: &Cx) -> Result<Response> {
    let user = settings_user(cx);
    app_context::<Dataserver>(cx)
        .purge_personal_storage(user.user_id)
        .await?;
    see_other(href!(super::storage).resolve(cx)).into_response(cx)
}
