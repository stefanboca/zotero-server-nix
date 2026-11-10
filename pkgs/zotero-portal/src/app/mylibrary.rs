use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::{SeeOther, see_other};
use topcoat::router::{href, route};

use crate::app::signed_in_user;

#[route(GET)]
async fn mylibrary(cx: &Cx) -> Result<SeeOther> {
    let user = signed_in_user(cx).await?;
    Ok(see_other(
        href!(
            super::_web_library::personal_library_root,
            super::_web_library::personal_user_slug: user.username,
        )
        .resolve(cx),
    ))
}
