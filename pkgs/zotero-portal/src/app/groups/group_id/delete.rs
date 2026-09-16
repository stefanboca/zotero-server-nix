use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::{forbidden, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use super::current_group;
use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

#[route(POST)]
pub(super) async fn delete_group(cx: &Cx) -> Result<Response> {
    let (actor_id, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    app_context::<Dataserver>(cx).delete_group(group.id).await?;
    app_context::<Database>(cx)
        .delete_group_invitations(group.id)
        .await?;
    tracing::info!(user_id = %actor_id, group_id = group.id, "group deleted");
    see_other(href!(crate::app::groups::list_groups).resolve(cx)).into_response(cx)
}
