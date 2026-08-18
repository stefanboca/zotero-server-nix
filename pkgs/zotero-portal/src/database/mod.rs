mod invitations;
mod sessions;
mod users;

pub use sessions::session_handle;

use std::{collections::BTreeMap, sync::Arc};

use anyhow::Context as _;
use jiff::Timestamp;
use sha2::{Digest as _, Sha256};
use sqlx::{
    ConnectOptions as _, MySql, MySqlPool, Transaction,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::{
    config::Config,
    dataserver::{Dataserver, WebLibraryAccess},
    id::UserId,
    oidc::OidcProfile,
};

pub struct Database {
    db: MySqlPool,
}

#[derive(Clone, Debug)]
pub struct UserSummary {
    pub user_id: UserId,
    pub username: String,
    pub email: String,
    pub real_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BrowserSession {
    pub handle: String,
    pub user_agent: Option<String>,
    pub last_seen_at: Timestamp,
    pub expires_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct GroupInvitation {
    pub group_id: u64,
    pub invitee: UserSummary,
    pub invited_at: Timestamp,
}

pub struct LoginCompletionLock<'a>(Transaction<'a, MySql>);

impl Database {
    pub async fn new(config: Arc<Config>) -> anyhow::Result<Self> {
        let db = MySqlPoolOptions::new()
            .max_connections(config.database.max_connections)
            .connect_with(MySqlConnectOptions::from_url(&config.database.url)?)
            .await
            .context("connecting to MariaDB")?;
        sqlx::migrate!()
            .run(&db)
            .await
            .context("running database migrations")?;
        Ok(Self { db })
    }
}
