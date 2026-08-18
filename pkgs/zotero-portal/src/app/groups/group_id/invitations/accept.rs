use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::{bad_request, not_found, see_other},
        href, path_param,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{
    app::authenticated_user,
    database::Database,
    dataserver::{Dataserver, GroupRole},
};

use super::super::{GroupId, group_by_id};

#[route(POST)]
pub(in crate::app::groups) async fn accept(cx: &Cx) -> Result<Response> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    let database = app_context::<Database>(cx);
    database
        .group_invitation(group_id, user_id)
        .await?
        .ok_or_else(not_found)?;
    let dataserver = app_context::<Dataserver>(cx);
    let group = group_by_id(cx, group_id).await?.ok_or_else(not_found)?;
    if group.role(user_id).is_some() {
        return Err(bad_request("you already belong to this group").into());
    }
    dataserver
        .set_group_user(group_id, user_id, GroupRole::Member)
        .await?;
    database.delete_group_invitation(group_id, user_id).await?;
    see_other(
        href!(
            crate::app::groups::group_id::group_slug::group_overview,
            GroupId(group.id),
            crate::app::groups::group_id::group_slug::GroupSlug(group.slug()),
        )
        .resolve(cx),
    )
    .into_response(cx)
}
