use topcoat::{
    Result,
    context::Cx,
    router::{Body, route},
};

use crate::{
    app::translation::proxy_translation,
    dataserver::{TranslationEndpoint, TranslationResponse},
};

#[route(POST)]
async fn export(cx: &Cx, body: Body) -> Result<TranslationResponse> {
    proxy_translation(cx, body, TranslationEndpoint::Export).await
}
