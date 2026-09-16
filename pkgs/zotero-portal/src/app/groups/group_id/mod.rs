mod delete;
mod group_slug;
mod invitations;
mod invite;
mod members;
mod owner;
mod settings;

pub(in crate::app) use group_slug::{GroupSlug, group_overview};
pub(in crate::app::groups) use invitations::{
    InviteeUserId, accept_invitation, cancel_invitation, ignore_invitation,
};
pub(super) use invite::invite_members;
pub(super) use settings::{library_settings, members_settings, settings as group_settings};
use topcoat::Result;
use topcoat::context::{Cx, app_context, memoize};
use topcoat::router::error::{not_found, see_other};
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, path_param, route};

use crate::app::authenticated_user;
use crate::dataserver::{Dataserver, Group, GroupRole};
use crate::id::UserId;

topcoat::router::path_param!(pub(in crate::app) group_id: u64, error = not_found);

#[memoize]
async fn memoized_group(cx: &Cx, group_id: u64) -> Result<Option<Group>> {
    app_context::<Dataserver>(cx)
        .group(group_id)
        .await
        .map_err(Into::into)
}

async fn group_by_id(cx: &Cx, group_id: u64) -> Result<Option<Group>> {
    match memoized_group(cx, group_id).await {
        Ok(group) => Ok(group.clone()),
        Err(error) => {
            tracing::error!(
                group_id,
                error = %format_args!("{error:#}"),
                "failed to load group",
            );
            Err(anyhow::anyhow!("failed to load group").into())
        }
    }
}

pub(super) async fn current_group(cx: &Cx) -> Result<(UserId, Group, GroupRole)> {
    let group_id = *path_param::<GroupId>(cx)?;
    let user_id = authenticated_user(cx).await?;
    let group = group_by_id(cx, group_id).await?.ok_or_else(not_found)?;
    let role = group.role(user_id).ok_or_else(not_found)?;
    Ok((user_id, group, role))
}

#[route(GET)]
async fn show_group(cx: &Cx) -> Result<Response> {
    let (_, group, _) = current_group(cx).await?;
    see_other(
        href!(
            group_slug::group_overview,
            GroupId(group.id),
            group_slug::GroupSlug(group.slug()),
        )
        .resolve(cx),
    )
    .into_response(cx)
}
