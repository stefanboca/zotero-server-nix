use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::page;
use topcoat::view::View;

topcoat::router::module_param!(pub(in crate::app) *library_path);

#[page]
pub(in crate::app) async fn group_library_path(cx: &Cx) -> Result<impl View> {
    super::group_library(cx).await
}
