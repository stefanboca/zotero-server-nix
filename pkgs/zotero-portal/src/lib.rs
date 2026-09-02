// Topcoat composes pages, layouts, and concurrent components into deeply nested
// async view futures. The default rustc query-depth limit is too small to lay
// out the generated route and view types in this application.
#![recursion_limit = "256"]

mod app;
mod config;
mod database;
mod dataserver;
mod id;
mod oidc;
mod provision;

use std::fmt::Write as _;

pub use app::serve;
pub use config::Config;
pub use provision::provision;

fn hex_string(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
