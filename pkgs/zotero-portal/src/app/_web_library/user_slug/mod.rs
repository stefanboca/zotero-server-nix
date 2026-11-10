mod items;
pub(super) mod library;
mod library_path;

pub(in crate::app) use items::personal_library_root;
use topcoat::Result;
use topcoat::context::Cx;
use topcoat::view::View;

use crate::app::_web_library::library;
use crate::app::signed_in_user;

topcoat::router::module_param!(pub(in crate::app) user_slug);

pub(super) async fn personal_library(cx: &Cx) -> Result<impl View> {
    let user = signed_in_user(cx).await?;
    if user_slug(cx) != user.username {
        return Err(topcoat::router::error::not_found().into());
    }
    library(cx).await
}
