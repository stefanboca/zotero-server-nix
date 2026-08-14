mod library_slug;

use topcoat::{
    Result,
    context::{Cx, CxBuilder, app_context},
    router::{Body, Next, Response, layer, page},
    session,
    view::{Unescaped, view},
};
use url::Url;

use crate::{
    app::{BareLayout, signed_in_user},
    database::{self, Database},
    dataserver::{Dataserver, WebLibraryAccess},
    id::UserId,
};

pub(crate) struct WebLibrary;

#[layer]
async fn bare_library_layout(cx: &mut CxBuilder, body: Body, next: Next<'_>) -> Result<Response> {
    cx.insert(BareLayout);
    cx.insert(WebLibrary);
    next.run(cx, body).await
}

#[page]
async fn library_page(cx: &Cx) -> Result {
    library(cx).await
}

pub(self) async fn library(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    let token_hash = session::token_hash(cx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("authenticated browser session disappeared"))?;
    let access = app_context::<Database>(cx)
        .web_library_access(app_context::<Dataserver>(cx), user.user_id, &*token_hash)
        .await?;
    let config = web_library_config(&user, access)?;

    view! {
        cx =>
        <main class="h-full w-full">
            <div class="h-full min-w-0 flex-1" id="zotero-web-library"></div>
            <script type="application/json" id="zotero-web-library-config">
                (config)
            </script>
            <script type="application/json" id="zotero-web-library-menu-config">
                (Unescaped::new_unchecked(r#"{"desktop":[],"mobile":[]}"#))
            </script>
            <script src="/static/web-library/zotero-web-library.js"></script>
        </main>
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLibraryConfig {
    is_embedded: bool,
    portal_mode: bool,
    read_only: bool,
    api_key: String,
    api_config: WebLibraryApiConfig,
    user_id: UserId,
    user_slug: String,
    libraries: WebLibraryLibraries,
    streaming_api_url: Url,
    is_recognizer_available: bool,
    website_url: String,
    base_path: String,
    buy_storage_url: String,
    styles_source_url: String,
    styles_base_url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLibraryApiConfig {
    api_scheme: String,
    api_authority_part: String,
    api_path: String,
    retry: u8,
    zotero_schema_version: u16,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLibraryLibraries {
    include_my_library: bool,
    include_user_groups: bool,
    include: Vec<serde_json::Value>,
}

fn web_library_config(
    user: &database::UserSummary,
    access: WebLibraryAccess,
) -> anyhow::Result<Unescaped<String>> {
    if !matches!(access.api_url.scheme(), "http" | "https") {
        anyhow::bail!("Web Library API URL must use HTTP or HTTPS");
    }
    if !matches!(access.streaming_url.scheme(), "ws" | "wss") {
        anyhow::bail!("Web Library streaming URL must use WebSocket");
    }
    let mut api_path = access.api_url.path().trim_start_matches('/').to_owned();
    if !api_path.ends_with('/') {
        api_path.push('/');
    }
    let website_url = format!("{}/", access.api_url.origin().ascii_serialization());
    let config = WebLibraryConfig {
        is_embedded: false,
        portal_mode: true,
        read_only: access.read_only,
        api_key: access.api_key,
        api_config: WebLibraryApiConfig {
            api_scheme: access.api_url.scheme().to_owned(),
            api_authority_part: access.api_url.authority().to_owned(),
            api_path,
            retry: 2,
            zotero_schema_version: 42, // TODO: get this dynamically
        },
        user_id: user.user_id,
        user_slug: user.username.clone(),
        libraries: WebLibraryLibraries {
            include_my_library: true,
            include_user_groups: true,
            include: Vec::new(),
        },
        streaming_api_url: access.streaming_url,
        is_recognizer_available: false,
        website_url,
        base_path: "/library".to_owned(),
        buy_storage_url: "/settings#storage".to_owned(),
        styles_source_url: "/static/web-library/styles/styles.json".to_owned(),
        styles_base_url: "/static/web-library/styles/".to_owned(),
    };
    let json = serde_json::to_string(&config)?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    Ok(Unescaped::new_unchecked(json))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> database::UserSummary {
        database::UserSummary {
            user_id: UserId::new(42),
            username: "researcher".to_owned(),
            email: "researcher@example.test".to_owned(),
            real_name: Some("Researcher".to_owned()),
        }
    }

    #[test]
    fn config_uses_the_configured_api_origin_and_path() {
        let config = web_library_config(
            &user(),
            WebLibraryAccess {
                api_key: "secret".to_owned(),
                api_url: Url::parse("https://zotero.example.test/api/").unwrap(),
                read_only: false,
                streaming_url: Url::parse("wss://zotero.example.test/stream/").unwrap(),
            },
        )
        .expect("valid Web Library configuration");

        assert!(config.contains(r#""apiScheme":"https""#));
        assert!(config.contains(r#""apiAuthorityPart":"zotero.example.test""#));
        assert!(config.contains(r#""apiPath":"api/""#));
        assert!(config.contains(r#""portalMode":true"#));
        assert!(config.contains(r#""readOnly":false"#));
        assert!(config.contains(r#""streamingApiUrl":"wss://zotero.example.test/stream/""#));
        assert!(config.contains(r#""stylesSourceUrl":"/static/web-library/styles/styles.json""#));
    }

    #[test]
    fn config_rejects_non_http_api_urls() {
        let result = web_library_config(
            &user(),
            WebLibraryAccess {
                api_key: "secret".to_owned(),
                api_url: Url::parse("file:///tmp/api").unwrap(),
                read_only: false,
                streaming_url: Url::parse("wss://zotero.example.test/stream/").unwrap(),
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn config_rejects_non_websocket_streaming_urls() {
        let result = web_library_config(
            &user(),
            WebLibraryAccess {
                api_key: "secret".to_owned(),
                api_url: Url::parse("https://zotero.example.test/api/").unwrap(),
                read_only: false,
                streaming_url: Url::parse("https://zotero.example.test/stream/").unwrap(),
            },
        );

        assert!(result.is_err());
    }
}
