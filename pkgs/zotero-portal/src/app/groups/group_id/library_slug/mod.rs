mod path;

use topcoat::router::path_param;

#[path_param(error = not_found)]
pub(super) struct LibrarySlug(pub(super) String);
