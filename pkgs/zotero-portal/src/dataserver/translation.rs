use anyhow::Context as _;
use reqwest::StatusCode;
use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::Body;
use topcoat::router::response::{IntoResponse, Response};

use super::Dataserver;

#[derive(Clone, Copy)]
pub enum TranslationEndpoint {
    Export,
    Import,
    Search,
    Web,
}

impl TranslationEndpoint {
    fn path(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
            Self::Search => "search",
            Self::Web => "web",
        }
    }
}

pub struct TranslationResponse {
    status: u16,
    body: Body,
    content_type: Option<String>,
    link: Option<String>,
}

impl IntoResponse for TranslationResponse {
    fn into_response(self, _: &Cx) -> Result<Response> {
        let mut builder = Response::builder().status(StatusCode::from_u16(self.status)?);
        if let Some(content_type) = self.content_type {
            builder = builder.header("content-type", content_type);
        }
        if let Some(link) = self.link {
            builder = builder.header("link", link);
        }
        Ok(builder
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .body(self.body)?)
    }
}

impl Dataserver {
    pub async fn translate(
        &self,
        endpoint: TranslationEndpoint,
        query: Option<&str>,
        content_type: Option<&str>,
        body: Body,
    ) -> anyhow::Result<TranslationResponse> {
        let mut url = self.config.translation_server_url.clone();
        url.set_path(endpoint.path());
        url.set_query(query);

        let mut request = self
            .client
            .post(url)
            .body(reqwest::Body::wrap_stream(body.into_data_stream()));
        if let Some(content_type) = content_type {
            request = request.header(reqwest::header::CONTENT_TYPE, content_type);
        }
        let response = request
            .send()
            .await
            .context("requesting Zotero translation")?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let link = response
            .headers()
            .get(reqwest::header::LINK)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = Body::new(reqwest::Body::from(response));
        Ok(TranslationResponse {
            status,
            body,
            content_type,
            link,
        })
    }
}
