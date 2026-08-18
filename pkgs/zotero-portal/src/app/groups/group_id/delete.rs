use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::{forbidden, see_other},
        href,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

use super::current_group;

#[route(POST)]
pub(super) async fn delete_group(cx: &Cx) -> Result<Response> {
    let (_, group, role) = current_group(cx).await?;
    if role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    app_context::<Dataserver>(cx).delete_group(group.id).await?;
    app_context::<Database>(cx)
        .delete_group_invitations(group.id)
        .await?;
    see_other(href!(crate::app::groups::list_groups).resolve(cx)).into_response(cx)
}
