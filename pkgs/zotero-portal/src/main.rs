use std::env;
use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use zotero_portal::{Config, provision, serve};

#[derive(Parser)]
struct Arguments {
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Provision {
        #[arg(long, value_name = "PATH")]
        state_directory: PathBuf,
    },
    Serve,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing()?;
    let arguments = Arguments::parse();
    let command = arguments.command.unwrap_or(Command::Serve);
    match command {
        Command::Provision { state_directory } => {
            provision(&state_directory).await?;
            Ok(())
        }
        Command::Serve => {
            let config = Config::new(arguments.config.as_deref())?;
            serve(config).await
        }
    }
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
