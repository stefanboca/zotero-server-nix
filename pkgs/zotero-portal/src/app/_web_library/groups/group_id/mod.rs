mod group_slug;

pub(in crate::app) use group_slug::{
    group_library_path, group_library_tail, group_slug as library_group_slug,
};

topcoat::router::module_param!(pub(in crate::app) group_id: u64 else not_found);
