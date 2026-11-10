use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, forbidden, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::super::current_group;
use crate::dataserver::{Dataserver, GroupRole};

#[route(POST)]
pub(in crate::app::groups::group_id) async fn leave_group(cx: &Cx) -> Result<SeeOther> {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    if actor_role == GroupRole::Owner {
        return Err(forbidden().into());
    }
    context::<Dataserver>(cx)
        .remove_group_user(group.id, actor_id)
        .await
        .map_err(Error::from_anyhow)?;
    Ok(see_other(
        href!(crate::app::groups::list_groups).resolve(cx),
    ))
}
