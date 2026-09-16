use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::see_other;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use crate::app::signed_in_user;
use crate::dataserver::Dataserver;

#[route(POST)]
pub(super) async fn purge_personal_storage(cx: &Cx) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    app_context::<Dataserver>(cx)
        .purge_personal_storage(user.user_id)
        .await?;
    tracing::info!(user_id = %user.user_id, "personal storage purged");
    see_other(href!(super::storage).resolve(cx)).into_response(cx)
}
