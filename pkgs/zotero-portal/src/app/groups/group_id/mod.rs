mod delete;
mod group_slug;
mod invitations;
mod invite;
mod members;
mod owner;
mod settings;

pub(in crate::app) use group_slug::{group_overview, group_slug as group_slug_param};
pub(in crate::app::groups) use invitations::{
    accept_invitation, cancel_invitation, ignore_invitation, invitee_user_id,
};
pub(super) use invite::invite_members;
pub(super) use settings::{library_settings, members_settings, settings as group_settings};
use topcoat::context::{Cx, context, memoize};
use topcoat::router::error::{SeeOther, not_found, see_other};
use topcoat::router::{href, route};
use topcoat::{Error, Result};

use crate::app::authenticated_user;
use crate::dataserver::{Dataserver, Group, GroupRole};
use crate::id::UserId;

topcoat::router::module_param!(pub(in crate::app) group_id: u64 else not_found);

#[memoize]
async fn memoized_group(cx: &Cx, group_id: u64) -> Result<Option<Group>> {
    context::<Dataserver>(cx)
        .group(group_id)
        .await
        .map_err(Error::from_anyhow)
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
            Err(topcoat::Error::msg("failed to load group"))
        }
    }
}

pub(super) async fn current_group(cx: &Cx) -> Result<(UserId, Group, GroupRole)> {
    let group_id = group_id(cx)?;
    let user_id = authenticated_user(cx).await?;
    let group = group_by_id(cx, *group_id).await?.ok_or_else(not_found)?;
    let role = group.role(user_id).ok_or_else(not_found)?;
    Ok((user_id, group, role))
}

#[route(GET)]
async fn show_group(cx: &Cx) -> Result<SeeOther> {
    let (_, group, _) = current_group(cx).await?;
    Ok(see_other(
        href!(
            group_slug::group_overview,
            group_id: group.id,
            group_slug::group_slug: group.slug(),
        )
        .resolve(cx),
    ))
}
