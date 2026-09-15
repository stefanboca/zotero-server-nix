mod library_path;

use topcoat::{Result, context::Cx, router::page, view::View};

#[page]
pub(in crate::app) async fn personal_library_root(cx: &Cx) -> Result<impl View> {
    super::personal_library(cx).await
}
