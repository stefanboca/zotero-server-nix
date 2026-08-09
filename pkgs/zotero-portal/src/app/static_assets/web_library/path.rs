use std::path::{Component, Path, PathBuf};

use topcoat::{
    Result,
    context::Cx,
    router::{Body, Response, error::not_found, raw_path_params, route},
};

topcoat::router::segment!(kind = CatchAll, rename = "path");

#[route(GET)]
async fn web_library_asset(cx: &Cx) -> Result<Response> {
    let relative = raw_path_params(cx)
        .into_iter()
        .find_map(|(name, value)| (name == "path").then_some(value))
        .ok_or_else(not_found)?;
    let relative = safe_relative_path(relative).ok_or_else(not_found)?;
    let root = Path::new(env!("ZOTERO_WEB_LIBRARY_DIR"));
    if root.as_os_str().is_empty() {
        return Err(not_found().into());
    }

    let path = root.join(&relative);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(not_found().into());
        }
        Err(error) => return Err(error.into()),
    };
    Ok(Response::builder()
        .header("content-type", content_type(&relative))
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))?)
}

fn safe_relative_path(value: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for component in Path::new(value).components() {
        match component {
            Component::Normal(component) => path.push(component),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => return None,
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("icc") => "application/vnd.iccprofile",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("md" | "txt") => "text/plain; charset=utf-8",
        Some("onnx") => "application/onnx",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("xml") => "application/xml; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nested_asset_paths() {
        assert_eq!(
            safe_relative_path("reader/images/loading.svg"),
            Some(PathBuf::from("reader/images/loading.svg"))
        );
    }

    #[test]
    fn rejects_paths_that_can_escape_the_asset_root() {
        assert_eq!(safe_relative_path("../secret"), None);
        assert_eq!(safe_relative_path("reader/../../secret"), None);
        assert_eq!(safe_relative_path("/etc/passwd"), None);
        assert_eq!(safe_relative_path(""), None);
    }

    #[test]
    fn uses_javascript_content_type_for_module_assets() {
        assert_eq!(
            content_type(Path::new("reader/reader.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type(Path::new("reader/reader.mjs")),
            "text/javascript; charset=utf-8"
        );
    }
}
