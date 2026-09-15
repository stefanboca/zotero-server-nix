use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
    view::View,
};

path_param!(*library_path);

#[page]
async fn personal_library_path(cx: &Cx) -> Result<impl View> {
    super::super::personal_library(cx).await
}
