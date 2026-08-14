mod app;
mod config;
mod database;
mod dataserver;
mod id;
mod oidc;

use std::fmt::Write as _;

pub use app::{migrate, routes, serve};
pub use config::{Config, Secrets};

fn hex_string(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
