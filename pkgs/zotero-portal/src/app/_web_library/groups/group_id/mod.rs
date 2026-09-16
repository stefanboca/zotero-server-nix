mod group_slug;

pub(in crate::app) use group_slug::{GroupSlug, LibraryPath, group_library_path};

topcoat::router::path_param!(pub(in crate::app) group_id: u64, error = not_found);
