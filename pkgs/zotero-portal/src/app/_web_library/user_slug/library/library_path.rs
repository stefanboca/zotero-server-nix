use topcoat::{
    Result,
    context::Cx,
    router::{page, path_param},
};

path_param!(*library_path);

#[page]
async fn personal_library_path(cx: &Cx) -> Result {
    super::super::personal_library(cx).await
}
