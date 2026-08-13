use topcoat::{
    Result,
    context::Cx,
    router::{error::not_found, page, uri},
};

use crate::app::library::user_web_library_document;

topcoat::router::segment!(kind = CatchAll, rename = "path");

#[page]
async fn fallback(cx: &Cx) -> Result {
    let path = uri(cx).path();
    let library_slug = path
        .split('/')
        .find(|segment| !segment.is_empty())
        .ok_or_else(not_found)?;
    user_web_library_document(cx, library_slug).await
}
