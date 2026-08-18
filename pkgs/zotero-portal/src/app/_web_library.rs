mod groups;
mod user_slug;

pub(in crate::app) use groups::{
    GroupId as LibraryGroupId, GroupSlug as LibraryGroupSlug, LibraryPath, group_library_path,
};
pub(in crate::app) use user_slug::{UserSlug, personal_library_root};

use serde_json::json;
use topcoat::{
    Result,
    context::{Cx, app_context, try_request_context},
    router::{Body, Next, href, layer, page, response::Response},
    session,
    view::{Unescaped, view},
};
use url::Url;

use crate::{
    app::signed_in_user,
    database::{self, Database},
    dataserver::{Dataserver, WebLibraryAccess},
    id::UserId,
};

pub(crate) struct WebLibrary;
struct GroupWebLibrary;

#[layer]
async fn web_library_request(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let cx = cx.with(WebLibrary);
    next.run(&cx, body).await
}

#[page]
pub(super) async fn landing(cx: &Cx) -> Result {
    library(cx).await
}

pub(super) async fn library(cx: &Cx) -> Result {
    let user = signed_in_user(cx).await?;
    let token_hash = session::token_hash(cx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("authenticated browser session disappeared"))?;
    let access = app_context::<Database>(cx)
        .web_library_access(app_context::<Dataserver>(cx), user.user_id, &*token_hash)
        .await?;
    let config = script_json(&WebLibraryConfig::new(
        &user,
        access,
        href!(crate::app::settings::storage_route).resolve(cx),
    )?)?;
    let menus = script_json(&web_library_menus(
        cx,
        &user,
        try_request_context::<GroupWebLibrary>(cx).is_some(),
    ))?;

    view! {
        cx =>
        <main class="h-dvh w-full overflow-hidden">
            <div class="h-full min-w-0 flex-1" id="zotero-web-library"></div>
            <script type="application/json" id="zotero-web-library-config">
                (config)
            </script>
            <script type="application/json" id="zotero-web-library-menu-config">
                (menus)
            </script>
            <script src="/static/web-library/zotero-web-library.js"></script>
        </main>
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLibraryConfig {
    is_embedded: bool,
    read_only: bool,
    api_key: String,
    api_config: WebLibraryApiConfig,
    user_id: UserId,
    user_slug: String,
    libraries: WebLibraryLibraries,
    streaming_api_url: Url,
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

impl WebLibraryConfig {
    fn new(
        user: &database::UserSummary,
        access: WebLibraryAccess,
        buy_storage_url: String,
    ) -> anyhow::Result<Self> {
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

        Ok(Self {
            is_embedded: false,
            read_only: access.read_only,
            api_key: access.api_key,
            api_config: WebLibraryApiConfig {
                api_scheme: access.api_url.scheme().to_owned(),
                api_authority_part: access.api_url.authority().to_owned(),
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
            website_url,
            buy_storage_url,
            styles_source_url: "/static/web-library/styles/styles.json".to_owned(),
            styles_base_url: "/static/web-library/styles/".to_owned(),
        })
    }
}

fn web_library_menus(
    cx: &Cx,
    user: &database::UserSummary,
    group_library: bool,
) -> serde_json::Value {
    let profile = href!(
        crate::app::profile::profile,
        crate::app::profile::ProfileUsername(user.username.clone()),
    )
    .resolve(cx);
    let groups = href!(crate::app::groups::list_groups).resolve(cx);
    let settings = href!(crate::app::settings::settings).resolve(cx);
    let logout = href!(crate::app::user::logout_route).resolve(cx);
    json!({
        "desktop": [
            {"label": "Groups", "href": groups, "active": group_library},
            {
                "label": user.display_name(),
                "truncate": true,
                "dropdown": true,
                "entries": [
                    {"label": "My Profile", "href": profile},
                    {"separator": true},
                    {"label": "Settings", "href": settings},
                    {"label": "Log Out", "href": logout}
                ]
            }
        ],
        "mobile": [
            {"label": "Groups", "href": groups, "active": group_library},
            {"label": user.display_name(), "href": profile, "className": "separated"},
            {"label": "Settings", "href": settings},
            {"label": "Log Out", "href": logout}
        ]
    })
}

fn script_json(value: &impl serde::Serialize) -> anyhow::Result<Unescaped<String>> {
    // Script elements are raw-text elements: HTML entities are not decoded in
    // their contents, so Topcoat's normal text escaping would produce invalid
    // JSON. Escaping `<` in the JSON itself prevents an embedded `</script>`
    // from terminating the element before the Web Library reads its text.
    let json = serde_json::to_string(value)?.replace('<', "\\u003c");
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
    fn config_uses_native_web_library_routes() {
        let config = WebLibraryConfig::new(
            &user(),
            WebLibraryAccess {
                api_key: "secret".to_owned(),
                api_url: Url::parse("https://zotero.example.test/api/").unwrap(),
                read_only: false,
                streaming_url: Url::parse("wss://zotero.example.test/stream/").unwrap(),
            },
            "/settings/storage".to_owned(),
        )
        .expect("valid Web Library configuration");
        let config = serde_json::to_string(&config).expect("serializable configuration");

        assert!(config.contains(r#""apiScheme":"https""#));
        assert!(config.contains(r#""apiPath":"api/""#));
        assert!(!config.contains("portalMode"));
        assert!(!config.contains("basePath"));
        assert!(config.contains(r#""readOnly":false"#));
    }
}
