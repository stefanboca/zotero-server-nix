use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::not_found;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, path_param, route};

use super::super::GroupId;
use crate::app::authenticated_user;
use crate::database::Database;

#[route(POST)]
pub(in crate::app::groups) async fn ignore(cx: &Cx) -> Result<Response> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    if !app_context::<Database>(cx)
        .delete_group_invitation(group_id, user_id)
        .await?
    {
        return Err(not_found().into());
    }
    topcoat::router::error::see_other(href!(crate::app::groups::list_groups).resolve(cx))
        .into_response(cx)
}
