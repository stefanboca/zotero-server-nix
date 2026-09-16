use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;

use crate::Config;
use crate::database::Database;

pub async fn provision(config: Option<Arc<Config>>, state_directory: &Path) -> anyhow::Result<()> {
    let provider = format!("file:{}", state_directory.display());
    let mut generated =
        secretspec::Secrets::load()?.with_reason("provision Zotero service secrets");
    generated.set_profile("production");
    generated.set_provider(provider);
    generated.set_scope("generated");
    generated.check(true)?;

    if let Some(config) = config {
        let mut database =
            secretspec::Secrets::load()?.with_reason("migrate the Zotero portal database");
        database.set_profile("production");
        database.set_scope("database");
        let database = database.resolve()?;
        let database_url = database
            .secrets
            .get("DATABASE_URL")
            .and_then(|secret| secret.value.as_deref())
            .context("DATABASE_URL was not resolved")?;
        tracing::info!("running portal database migrations");
        Database::new(config, database_url.trim())
            .await
            .context("provisioning the portal database")?;
        tracing::info!("portal database migrations complete");
    }
    Ok(())
}
