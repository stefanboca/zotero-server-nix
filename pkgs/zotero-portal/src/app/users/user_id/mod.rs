mod items;

use topcoat::router::path_param;

#[path_param(error = not_found)]
pub(super) struct UserIdPath(pub(super) u64);
