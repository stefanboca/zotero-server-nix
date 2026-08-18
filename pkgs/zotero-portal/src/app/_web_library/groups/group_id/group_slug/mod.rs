mod library_path;

pub(in crate::app) use library_path::{LibraryPath, group_library_path};

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::not_found, path_param},
};

use crate::{
    app::{
        _web_library::{GroupWebLibrary, library},
        authenticated_user,
    },
    dataserver::Dataserver,
};

use super::GroupId;

topcoat::router::path_param!(pub(in crate::app) group_slug);

async fn group_library(cx: &Cx) -> Result {
    let user_id = authenticated_user(cx).await?;
    let group_id = *path_param::<GroupId>(cx)?;
    let group = app_context::<Dataserver>(cx)
        .group(group_id)
        .await?
        .ok_or_else(not_found)?;
    if group.role(user_id).is_none() || path_param::<GroupSlug>(cx) != group.slug() {
        return Err(not_found().into());
    }
    let cx = cx.with(GroupWebLibrary);
    library(&cx).await
}
