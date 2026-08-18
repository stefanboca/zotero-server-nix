mod delete;
mod group_slug;
mod members;
mod owner;
mod settings;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        error::{not_found, see_other},
        href, path_param,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{
    app::authenticated_user,
    dataserver::{Dataserver, Group, GroupRole},
    id::UserId,
};

pub(super) use settings::{library_settings, members_settings, settings as group_settings};

topcoat::router::path_param!(pub(super) group_id: u64, error = not_found);

pub(super) async fn current_group(cx: &Cx) -> Result<(UserId, Group, GroupRole)> {
    let group_id = *path_param::<GroupId>(cx)?;
    let user_id = authenticated_user(cx).await?;
    let dataserver = app_context::<Dataserver>(cx);
    let group = dataserver.group(group_id).await?.ok_or_else(not_found)?;
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
