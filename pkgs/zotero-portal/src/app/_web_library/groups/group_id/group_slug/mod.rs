mod library_path;

pub(in crate::app) use library_path::{group_library_path, library_path as group_library_tail};
use topcoat::context::{Cx, context};
use topcoat::router::error::not_found;
use topcoat::view::View;
use topcoat::{Error, Result};

use super::group_id;
use crate::app::_web_library::{GroupWebLibrary, library};
use crate::app::authenticated_user;
use crate::dataserver::Dataserver;

topcoat::router::module_param!(pub(in crate::app) group_slug);

async fn group_library(cx: &Cx) -> Result<impl View> {
    let user_id = authenticated_user(cx).await?;
    let group_id = group_id(cx)?;
    let group = context::<Dataserver>(cx)
        .group(*group_id)
        .await
        .map_err(Error::from_anyhow)?
        .ok_or_else(not_found)?;
    if group.role(user_id).is_none() || group_slug(cx) != group.slug() {
        return Err(not_found().into());
    }
    let cx = cx.with(GroupWebLibrary);
    library(&cx).await
}
