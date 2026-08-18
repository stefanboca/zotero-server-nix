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
    let ansi = match env::var("ZOTERO_PORTAL_LOG_ANSI") {
        Ok(value) if matches!(value.as_str(), "1" | "true") => true,
        Ok(value) if matches!(value.as_str(), "0" | "false") => false,
        Ok(value) => bail!("invalid ZOTERO_PORTAL_LOG_ANSI value: {value}"),
        Err(env::VarError::NotPresent) => false,
        Err(error) => return Err(error.into()),
    };
    let format = env::var("ZOTERO_PORTAL_LOG_FORMAT").unwrap_or_else(|_| "compact".to_owned());
    match format.as_str() {
        "journald" => tracing_subscriber::registry()
            .with(filter)
            .with(tracing_journald::layer()?)
            .try_init()
            .map_err(|error| anyhow!("initializing tracing subscriber: {error}"))?,
        "json" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(ansi)
            .json()
            .try_init()
            .map_err(|error| anyhow!("initializing tracing subscriber: {error}"))?,
        "pretty" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(ansi)
            .pretty()
            .try_init()
            .map_err(|error| anyhow!("initializing tracing subscriber: {error}"))?,
        "compact" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(ansi)
            .compact()
            .try_init()
            .map_err(|error| anyhow!("initializing tracing subscriber: {error}"))?,
        value => bail!("invalid ZOTERO_PORTAL_LOG_FORMAT value: {value}"),
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
