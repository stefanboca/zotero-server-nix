mod leave;

use std::collections::HashSet;

pub(super) use leave::leave_group;
use serde::{Deserialize, Serialize};
use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::content::Json;
use topcoat::router::error::{bad_request, forbidden, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use super::{GroupId, current_group};
use crate::database::Database;
use crate::dataserver::{Dataserver, Group, GroupRole};
use crate::id::UserId;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMember {
    identifier: String,
    role: GroupRole,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum MemberRoleUpdate {
    Admin,
    Member,
    Remove,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MemberUpdate {
    user_id: UserId,
    role: MemberRoleUpdate,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateMembers {
    members: Vec<MemberUpdate>,
}

#[derive(Serialize)]
struct UpdatedMembers {
    updated: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MemberResponse {
    user_id: UserId,
    username: String,
    role: String,
}

pub(super) async fn managed_group(cx: &Cx) -> Result<Group> {
    let (_, group, role) = current_group(cx).await?;
    if !role.can_manage() {
        return Err(forbidden().into());
    }
    Ok(group)
}

#[route(GET)]
async fn members(cx: &Cx) -> Result<Response> {
    let (_, group, _) = current_group(cx).await?;
    see_other(href!(super::settings::members_settings, GroupId(group.id)).resolve(cx))
        .into_response(cx)
}

#[route(POST)]
pub(super) async fn create_member(
    cx: &Cx,
    Json(input): Json<CreateMember>,
) -> Result<Json<MemberResponse>> {
    if !matches!(input.role, GroupRole::Admin | GroupRole::Member) {
        return Err(bad_request("a new member must be an administrator or member").into());
    }
    let identifier = input.identifier.trim();
    let identifier = identifier.strip_prefix('@').unwrap_or(identifier);
    if identifier.is_empty() {
        return Err(bad_request("a username or email is required").into());
    }
    let group = managed_group(cx).await?;
    let target = app_context::<Database>(cx)
        .user_by_identifier(identifier)
        .await?
        .ok_or_else(|| bad_request("no account matched that username or email"))?;
    if group.role(target.user_id).is_some() {
        return Err(bad_request("that account is already a group member").into());
    }
    app_context::<Dataserver>(cx)
        .set_group_user(group.id, target.user_id, input.role)
        .await?;
    tracing::info!(
        group_id = group.id,
        user_id = %target.user_id,
        role = %input.role,
        "group member added",
    );
    Ok(Json(member_response(
        target.user_id,
        target.username,
        input.role,
    )))
}

#[route(PATCH)]
async fn update_members(cx: &Cx, Json(input): Json<UpdateMembers>) -> Result<Json<UpdatedMembers>> {
    if input.members.is_empty() {
        return Err(bad_request("at least one member update is required").into());
    }
    let group = managed_group(cx).await?;
    let mut seen = HashSet::with_capacity(input.members.len());
    for update in &input.members {
        if !seen.insert(update.user_id) {
            return Err(bad_request("a member can only be updated once per request").into());
        }
        match group.role(update.user_id) {
            Some(GroupRole::Admin | GroupRole::Member) => {}
            Some(GroupRole::Owner) => return Err(forbidden().into()),
            None => return Err(bad_request("an update targeted a non-member").into()),
        }
    }

    let dataserver = app_context::<Dataserver>(cx);
    for update in &input.members {
        match update.role {
            MemberRoleUpdate::Admin => {
                dataserver
                    .set_group_user(group.id, update.user_id, GroupRole::Admin)
                    .await?;
            }
            MemberRoleUpdate::Member => {
                dataserver
                    .set_group_user(group.id, update.user_id, GroupRole::Member)
                    .await?;
            }
            MemberRoleUpdate::Remove => {
                dataserver
                    .remove_group_user(group.id, update.user_id)
                    .await?;
            }
        }
    }

    tracing::info!(
        group_id = group.id,
        updated = input.members.len(),
        "group member roles updated",
    );

    Ok(Json(UpdatedMembers {
        updated: input.members.len(),
    }))
}

pub(super) fn member_response(
    user_id: UserId,
    username: String,
    role: GroupRole,
) -> MemberResponse {
    MemberResponse {
        user_id,
        username,
        role: role.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_updates_use_typed_integer_ids_and_closed_roles() {
        let input: UpdateMembers = serde_json::from_value(serde_json::json!({
            "members": [
                {"userId": 42, "role": "admin"},
                {"userId": 43, "role": "remove"}
            ]
        }))
        .expect("valid member updates must deserialize");

        assert_eq!(input.members[0].user_id, UserId::new(42));
        assert_eq!(input.members[0].role, MemberRoleUpdate::Admin);
        assert_eq!(input.members[1].role, MemberRoleUpdate::Remove);
        assert!(
            serde_json::from_value::<UpdateMembers>(serde_json::json!({
                "members": [{"userId": 42.5, "role": "member"}]
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<UpdateMembers>(serde_json::json!({
                "members": [{"userId": 42, "role": "owner"}]
            }))
            .is_err()
        );
    }
}
