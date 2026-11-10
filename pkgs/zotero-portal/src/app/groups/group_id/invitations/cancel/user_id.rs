use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, not_found};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::super::super::group_id;
use super::super::super::members::managed_group;
use crate::database::Database;
use crate::id::UserId;

topcoat::router::module_param!(
    pub(in crate::app::groups) invitee_user_id: UserId else not_found
);

#[route(POST)]
pub(in crate::app::groups) async fn cancel(cx: &Cx) -> Result<SeeOther> {
    let group = managed_group(cx).await?;
    let invitee_id = invitee_user_id(cx)?;
    if !context::<Database>(cx)
        .delete_group_invitation(group.id, *invitee_id)
        .await
        .map_err(Error::from_anyhow)?
    {
        return Err(not_found().into());
    }
    Ok(topcoat::router::error::see_other(
        href!(crate::app::groups::group_id::members_settings, group_id: group.id).resolve(cx),
    ))
}
