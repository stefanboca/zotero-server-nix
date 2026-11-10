use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::page;
use topcoat::view::View;

topcoat::router::module_param!(*library_path);

#[page]
async fn personal_library_path(cx: &Cx) -> Result<impl View> {
    super::super::personal_library(cx).await
}
