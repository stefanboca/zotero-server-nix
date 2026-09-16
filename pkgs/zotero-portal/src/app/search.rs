use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::{Body, route};

use crate::app::translation::proxy_translation;
use crate::dataserver::{TranslationEndpoint, TranslationResponse};

#[route(POST)]
async fn search(cx: &Cx, body: Body) -> Result<TranslationResponse> {
    proxy_translation(cx, body, TranslationEndpoint::Search).await
}
