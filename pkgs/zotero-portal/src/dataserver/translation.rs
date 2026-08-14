use anyhow::Context as _;

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
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub link: Option<String>,
}

impl Dataserver {
    pub async fn translate(
        &self,
        endpoint: TranslationEndpoint,
        query: Option<&str>,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> anyhow::Result<TranslationResponse> {
        let mut url = self.config.translation_server_url.clone();
        url.set_path(endpoint.path());
        url.set_query(query);

        let mut request = self.client.post(url).body(body);
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
        let body = response
            .bytes()
            .await
            .context("reading the Zotero translation response")?
            .to_vec();
        Ok(TranslationResponse {
            status,
            body,
            content_type,
            link,
        })
    }
}
