use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
};

use crate::app::library::web_library_document;

use super::{super::current_group, LibrarySlug};

topcoat::router::segment!(kind = CatchAll, rename = "path");

#[page]
async fn group_library(cx: &Cx) -> Result {
    path_param::<LibrarySlug>(cx)?;
    current_group(cx).await?;
    web_library_document(cx).await
}
