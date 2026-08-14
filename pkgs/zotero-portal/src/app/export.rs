use topcoat::{
    Result,
    context::Cx,
    router::{Body, Response, route},
};

use crate::{app::translation::proxy_translation, dataserver::TranslationEndpoint};

#[route(POST)]
async fn export(cx: &Cx, body: Body) -> Result<Response> {
    proxy_translation(cx, body, TranslationEndpoint::Export).await
}
