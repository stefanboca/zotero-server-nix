use std::{env, path::PathBuf};

use anyhow::{Context as _, anyhow, bail};
use zotero_portal::{Config, migrate, serve, serve_development};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if env::var_os("TOPCOAT_DEV_URL").is_some() {
        return serve_development()
            .await
            .context("UI development server failed");
    }

    let (config_path, migrate_only) = arguments()?;
    let config = Config::from_file(&config_path)?;

    if migrate_only {
        migrate(&config).await
    } else {
        serve(config).await
    }
}

fn arguments() -> anyhow::Result<(PathBuf, bool)> {
    let mut arguments = env::args().skip(1);
    let mut config_path = None;
    let mut migrate_only = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--config" => {
                config_path = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| anyhow!("--config requires a path"))?,
                ));
            }
            "--migrate-only" => migrate_only = true,
            _ => bail!("unknown argument: {argument}"),
        }
    }
    Ok((
        config_path.ok_or_else(|| anyhow!("--config is required"))?,
        migrate_only,
    ))
}
