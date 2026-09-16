use topcoat::Result;
use topcoat::context::Cx;
use topcoat::router::error::not_found;
use topcoat::router::{page, path_param};
use topcoat::view::View;

topcoat::router::path_param!(*personal_library_path);

/// Render client-side Web Library routes on a direct request or refresh while
/// leaving unrelated paths available for portal routes and normal 404s.
#[page]
async fn personal_library_path(cx: &Cx) -> Result<impl View> {
    let mut path = path_param::<PersonalLibraryPath>(cx);
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
