use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{Body, Response, StatusCode, headers, to_bytes, uri},
};

use crate::{
    app::authenticated_user,
    dataserver::{Dataserver, TranslationEndpoint},
};

const MAX_TRANSLATION_BODY_SIZE: usize = 5 * 1024 * 1024;

pub(super) async fn proxy_translation(
    cx: &Cx,
    body: Body,
    endpoint: TranslationEndpoint,
) -> Result<Response> {
    authenticated_user(cx).await?;
    let content_type = headers(cx)
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let query = uri(cx).query().map(str::to_owned);
    let body = to_bytes(body, MAX_TRANSLATION_BODY_SIZE)
        .await
        .map_err(|error| anyhow::anyhow!("reading translation request: {error}"))?
        .to_vec();
    let response = app_context::<Dataserver>(cx)
        .translate(endpoint, query.as_deref(), content_type.as_deref(), body)
        .await?;

    let mut builder = Response::builder().status(StatusCode::from_u16(response.status)?);
    if let Some(content_type) = response.content_type {
        builder = builder.header("content-type", content_type);
    }
    if let Some(link) = response.link {
        builder = builder.header("link", link);
    }
    Ok(builder
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(response.body))?)
}
