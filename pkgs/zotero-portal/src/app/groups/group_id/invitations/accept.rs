use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, bad_request, not_found, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::super::{group_by_id, group_id};
use crate::app::authenticated_user;
use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

#[route(POST)]
pub(in crate::app::groups) async fn accept(cx: &Cx) -> Result<SeeOther> {
    let user_id = authenticated_user(cx).await?;
    let group_id = *group_id(cx)?;
    let database = context::<Database>(cx);
    database
        .group_invitation(group_id, user_id)
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(not_found)?;
    let dataserver = context::<Dataserver>(cx);
    let group = group_by_id(cx, group_id).await?.ok_or_else(not_found)?;
    if group.role(user_id).is_some() {
        return Err(bad_request("you already belong to this group").into());
    }
    dataserver
        .set_group_user(group_id, user_id, GroupRole::Member)
        .await
        .map_err(Error::from_anyhow)?;
    database
        .delete_group_invitation(group_id, user_id)
        .await
        .map_err(Error::from_anyhow)?;
    Ok(see_other(
        href!(
            crate::app::groups::group_id::group_slug::group_overview,
            group_id: group.id,
            crate::app::groups::group_id::group_slug::group_slug: group.slug(),
        )
        .resolve(cx),
    ))
}
