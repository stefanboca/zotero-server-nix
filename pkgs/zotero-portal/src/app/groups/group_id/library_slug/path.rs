use topcoat::{
    Result,
    context::Cx,
    router::{error::not_found, page, path_param, uri},
};

use crate::app::{is_web_library_path, library::web_library_document};

use super::{LibrarySlug, super::current_group};

topcoat::router::segment!(kind = CatchAll, rename = "path");

#[page]
async fn group_library(cx: &Cx) -> Result {
    if !is_web_library_path(uri(cx).path()) {
        return Err(not_found().into());
    }
    path_param::<LibrarySlug>(cx)?;
    current_group(cx).await?;
    web_library_document(cx).await
}
