mod library_path;

use topcoat::{Result, context::Cx, router::page};

#[page]
pub(in crate::app) async fn personal_library_root(cx: &Cx) -> Result {
    super::personal_library(cx).await
}
