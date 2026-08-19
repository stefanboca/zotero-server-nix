use std::{env, path::PathBuf};

use anyhow::{anyhow, bail};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};
use zotero_portal::{Config, Secrets, migrate, serve};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;
    let (config, secrets) = if env::var_os("TOPCOAT_DEV_URL").is_some() {
        tracing::info!("starting with development configuration");
        (Config::dev(), Secrets::dev())
    } else {
        let (config_path, migrate_only) = arguments()?;
        tracing::info!(
            config = %config_path.display(),
            migrate_only,
            "loading configuration",
        );
        let config = Config::from_file(&config_path)?;

        if migrate_only {
            return migrate(config).await;
        }
        (config, Secrets::from_systemd()?)
    };

    serve(config, secrets).await
}

fn init_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("zotero_portal=info,tower_http=info"));
    if env::var_os("JOURNAL_STREAM").is_some() {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_journald::layer()?)
            .try_init()
            .map_err(|error| anyhow!("initializing journald tracing: {error}"))?;
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .try_init()
            .map_err(|error| anyhow!("initializing terminal tracing: {error}"))?;
    }
    Ok(())
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
