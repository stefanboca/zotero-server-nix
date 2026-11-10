use topcoat::context::{Cx, context};
use topcoat::router::error::{SeeOther, forbidden, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use super::current_group;
use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

#[route(POST)]
pub(super) async fn delete_group(cx: &Cx) -> Result<SeeOther> {
    let (actor_id, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    context::<Dataserver>(cx)
        .delete_group(group.id)
        .await
        .map_err(Error::from_anyhow)?;
    context::<Database>(cx)
        .delete_group_invitations(group.id)
        .await
        .map_err(Error::from_anyhow)?;
    tracing::info!(user_id = %actor_id, group_id = group.id, "group deleted");
    Ok(see_other(
        href!(crate::app::groups::list_groups).resolve(cx),
    ))
}
