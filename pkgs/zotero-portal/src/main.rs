use std::{env, path::PathBuf};

use anyhow::{anyhow, bail};
use zotero_portal::{Config, Secrets, migrate, serve};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (config, secrets) = if env::var_os("TOPCOAT_DEV_URL").is_some() {
        (Config::dev(), Secrets::dev())
    } else {
        let (config_path, migrate_only) = arguments()?;
        let config = Config::from_file(&config_path)?;

        if migrate_only {
            return migrate(config).await;
        }
        (config, Secrets::from_systemd()?)
    };

    serve(config, secrets).await
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
