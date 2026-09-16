use topcoat::Result;
use topcoat::context::{Cx, app_context};
use topcoat::router::Body;
use topcoat::router::request::{headers, uri};

use crate::app::authenticated_user;
use crate::dataserver::{Dataserver, TranslationEndpoint, TranslationResponse};

pub(super) async fn proxy_translation(
    cx: &Cx,
    body: Body,
    endpoint: TranslationEndpoint,
) -> Result<TranslationResponse> {
    authenticated_user(cx).await?;
    let content_type = headers(cx)
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let query = uri(cx).query().map(str::to_owned);
    Ok(app_context::<Dataserver>(cx)
        .translate(endpoint, query.as_deref(), content_type.as_deref(), body)
        .await?)
}
