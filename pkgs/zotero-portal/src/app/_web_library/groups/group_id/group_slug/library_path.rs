use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
    view::View,
};

path_param!(pub(in crate::app) *library_path);

#[page]
pub(in crate::app) async fn group_library_path(cx: &Cx) -> Result<impl View> {
    super::group_library(cx).await
}
