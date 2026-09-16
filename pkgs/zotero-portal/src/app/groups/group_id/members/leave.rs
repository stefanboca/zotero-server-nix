use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::error::{forbidden, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use super::super::current_group;
use crate::dataserver::{Dataserver, GroupRole};

#[route(POST)]
pub(in crate::app::groups::group_id) async fn leave_group(cx: &Cx) -> Result<Response> {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    if actor_role == GroupRole::Owner {
        return Err(forbidden().into());
    }
    app_context::<Dataserver>(cx)
        .remove_group_user(group.id, actor_id)
        .await?;
    see_other(href!(crate::app::groups::list_groups).resolve(cx)).into_response(cx)
}
