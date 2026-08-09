use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{Body, Response, StatusCode, error::not_found, headers, raw_path_params, route, uri},
};

use crate::app::AppState;

topcoat::router::segment!(kind = CatchAll, rename = "path");

#[route(GET)]
async fn development_api(cx: &Cx) -> Result<Response> {
    let path = raw_path_params(cx)
        .into_iter()
        .find_map(|(name, value)| (name == "path").then_some(value))
        .ok_or_else(not_found)?;
    let api_key_from_query = uri(cx).query().and_then(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .find_map(|(name, value)| (name == "key").then(|| value.into_owned()))
    });
    let api_key_from_header = headers(cx)
        .get("zotero-api-key")
        .and_then(|value| value.to_str().ok());
    let api_key = api_key_from_header
        .map(str::to_owned)
        .or(api_key_from_query)
        .ok_or_else(not_found)?;
    let response = app_context::<AppState>(cx)
        .backend
        .web_library_api_response(&api_key, path, uri(cx).query())
        .await?
        .ok_or_else(not_found)?;

    let mut builder = Response::builder()
        .status(StatusCode::from_u16(response.status)?)
        .header("content-type", response.content_type)
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff");
    if let Some(total_results) = response.total_results {
        builder = builder.header("total-results", total_results);
    }
    if let Some(version) = response.last_modified_version {
        builder = builder.header("last-modified-version", version);
    }
    let body = match response.raw_body {
        Some(body) => body,
        None => serde_json::to_vec(&response.body)?,
    };
    Ok(builder.body(Body::from(body))?)
}
