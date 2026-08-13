mod app;
mod backend;
mod config;
mod dataserver;
mod id;
mod oidc;

use std::{
    fmt::Write as _,
    time::{SystemTime, UNIX_EPOCH},
};

pub use app::{migrate, routes, serve};
pub use config::{Config, Secrets};

fn epoch_seconds() -> anyhow::Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn hex_string(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
