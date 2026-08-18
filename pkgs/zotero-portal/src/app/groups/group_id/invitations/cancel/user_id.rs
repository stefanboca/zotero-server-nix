use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::not_found,
        href, path_param,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{database::Database, id::UserId};

use super::super::super::{GroupId, members::managed_group};

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
