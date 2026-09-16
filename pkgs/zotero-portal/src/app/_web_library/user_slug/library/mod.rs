mod library_path;

use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::page;
use topcoat::view::View;

#[page]
pub(in crate::app) async fn personal_library_root(cx: &Cx) -> Result<impl View> {
    super::personal_library(cx).await
}
