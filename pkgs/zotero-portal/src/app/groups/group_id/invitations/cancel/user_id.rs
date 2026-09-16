use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::not_found;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, path_param, route};

use super::super::super::GroupId;
use super::super::super::members::managed_group;
use crate::database::Database;
use crate::id::UserId;

topcoat::router::path_param!(pub(in crate::app::groups) invitee_user_id: UserId, error = not_found);

#[route(POST)]
pub(in crate::app::groups) async fn cancel(cx: &Cx) -> Result<Response> {
    let group = managed_group(cx).await?;
    let invitee_id = *path_param::<InviteeUserId>(cx)?;
    if !app_context::<Database>(cx)
        .delete_group_invitation(group.id, invitee_id)
        .await?
    {
        return Err(not_found().into());
    }
    topcoat::router::error::see_other(
        href!(
            crate::app::groups::group_id::members_settings,
            GroupId(group.id),
        )
        .resolve(cx),
    )
    .into_response(cx)
}
