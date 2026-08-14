use topcoat::{Result, context::Cx, router::page};

topcoat::router::segment!(kind = CatchAll);

#[page]
async fn library_path(cx: &Cx) -> Result {
    super::library(cx).await
}
