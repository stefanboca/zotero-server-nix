use serde::Deserialize;
use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::content::Form;
use topcoat::router::error::{bad_request, forbidden, not_found, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use super::{GroupId, current_group};
use crate::database::Database;
use crate::dataserver::{Dataserver, GroupRole};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferOwner {
    username: String,
}

#[route(POST)]
pub(super) async fn transfer_owner(cx: &Cx, Form(input): Form<TransferOwner>) -> Result<Response> {
    let (actor_id, group, actor_role) = current_group(cx).await?;
    if actor_role != GroupRole::Owner {
        return Err(forbidden().into());
    }
    let database = app_context::<Database>(cx);
    let target = database
        .user_by_identifier(&input.username)
        .await?
        .ok_or_else(not_found)?;
    match group.role(target.user_id) {
        Some(GroupRole::Admin | GroupRole::Member) => {}
        Some(GroupRole::Owner) => {
            return Err(bad_request("that user already owns the group").into());
        }
        None => return Err(bad_request("the new owner must already be a group member").into()),
    }
    app_context::<Dataserver>(cx)
        .set_group_user(group.id, target.user_id, GroupRole::Owner)
        .await?;
    tracing::info!(
        group_id = group.id,
        previous_owner_id = %actor_id,
        new_owner_id = %target.user_id,
        "group ownership transferred",
    );
    see_other(href!(super::settings::members_settings, GroupId(group.id)).resolve(cx))
        .into_response(cx)
}
