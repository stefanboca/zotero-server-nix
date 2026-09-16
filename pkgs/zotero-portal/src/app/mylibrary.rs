use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::see_other;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{href, route};

use crate::app::signed_in_user;

#[route(GET)]
async fn mylibrary(cx: &Cx) -> Result<Response> {
    let user = signed_in_user(cx).await?;
    see_other(
        href!(
            super::_web_library::personal_library_root,
            super::_web_library::UserSlug(user.username),
        )
        .resolve(cx),
    )
    .into_response(cx)
}
