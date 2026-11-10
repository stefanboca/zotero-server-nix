use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::not_found;
use topcoat::router::page;
use topcoat::view::View;

topcoat::router::module_param!(*personal_library_path);

/// Render client-side Web Library routes on a direct request or refresh while
/// leaving unrelated paths available for portal routes and normal 404s.
#[page]
async fn personal_library_path_(cx: &Cx) -> Result<impl View> {
    let mut path = personal_library_path(cx);
    let Some(prefix) = path.next() else {
        return Err(not_found().into());
    };
    if ![
        "collections",
        "items",
        "library",
        "publications",
        "search",
        "tags",
        "trash",
    ]
    .contains(&prefix)
    {
        return Err(not_found().into());
    }
    super::personal_library(cx).await
}
