use anyhow::{Context as _, bail};
use topcoat::{
    Result,
    context::{Cx, CxBuilder, app_context},
    router::{Body, Next, Response, layer, page},
    view::{Unescaped, view},
};

use crate::{
    app::{AppState, BareLayout, signed_in_user},
    backend::WebLibraryAccess,
    id::UserId,
    store,
};

#[layer]
async fn bare_library_layout(cx: &mut CxBuilder, body: Body, next: Next<'_>) -> Result<Response> {
    cx.insert(BareLayout);
    next.run(cx, body).await
}

#[page]
async fn library(cx: &Cx) -> Result {
    web_library_document(cx).await
}

pub(super) async fn web_library_document(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    render_web_library_document(cx, &user).await
}

pub(super) async fn user_web_library_document(cx: &Cx, library_slug: &str) -> Result {
    let user = signed_in_user(cx).await?;
    if library_slug != user.username {
        return Err(topcoat::router::error::not_found().into());
    }
    render_web_library_document(cx, &user).await
}

async fn render_web_library_document(cx: &Cx, user: &store::UserSummary) -> Result {
    let access = app_context::<AppState>(cx)
        .backend
        .web_library_access(user.user_id)
        .await?;
    let config = web_library_config(user, access)?;

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
    streaming_api_url: String,
    is_recognizer_available: bool,
    website_url: String,
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
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLibraryLibraries {
    include_my_library: bool,
    include_user_groups: bool,
    include: Vec<serde_json::Value>,
}

fn web_library_config(
    user: &store::UserSummary,
    access: WebLibraryAccess,
) -> anyhow::Result<Unescaped<String>> {
    let api_url = url::Url::parse(&access.api_url).context("parsing the public Zotero API URL")?;
    if !matches!(api_url.scheme(), "http" | "https")
        || api_url.host_str().is_none()
        || !api_url.username().is_empty()
        || api_url.password().is_some()
        || api_url.query().is_some()
        || api_url.fragment().is_some()
    {
        bail!("the public Zotero API URL must be an HTTP URL without credentials or a query");
    }
    if !access.streaming_url.is_empty() {
        let streaming_url = url::Url::parse(&access.streaming_url)
            .context("parsing the public Zotero streaming URL")?;
        if !matches!(streaming_url.scheme(), "ws" | "wss")
            || streaming_url.host_str().is_none()
            || !streaming_url.username().is_empty()
            || streaming_url.password().is_some()
            || streaming_url.query().is_some()
            || streaming_url.fragment().is_some()
        {
            bail!(
                "the public Zotero streaming URL must be a WebSocket URL without credentials or a query"
            );
        }
    }

    let origin = api_url.origin().ascii_serialization();
    let authority = origin
        .strip_prefix(&format!("{}://", api_url.scheme()))
        .context("the public Zotero API URL has no authority")?
        .to_owned();
    let mut api_path = api_url.path().trim_start_matches('/').to_owned();
    if !api_path.ends_with('/') {
        api_path.push('/');
    }
    let website_url = format!("{origin}/");
    let config = WebLibraryConfig {
        is_embedded: false,
        portal_mode: true,
        read_only: access.read_only,
        api_key: access.api_key,
        api_config: WebLibraryApiConfig {
            api_scheme: api_url.scheme().to_owned(),
            api_authority_part: authority,
            api_path,
            retry: 2,
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
        buy_storage_url: format!("{website_url}settings/storage"),
        website_url,
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

    fn user() -> store::UserSummary {
        store::UserSummary {
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
                api_url: "https://zotero.example.test/api/".to_owned(),
                read_only: false,
                streaming_url: "wss://zotero.example.test/stream/".to_owned(),
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
                api_url: "file:///tmp/api".to_owned(),
                read_only: false,
                streaming_url: "wss://zotero.example.test/stream/".to_owned(),
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
                api_url: "https://zotero.example.test/api/".to_owned(),
                read_only: false,
                streaming_url: "https://zotero.example.test/stream/".to_owned(),
            },
        );

        assert!(result.is_err());
    }
}
