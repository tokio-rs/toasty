#![warn(missing_docs)]

use async_trait::async_trait;
use std::{borrow::Cow, fmt, path::Path, sync::Arc};
use toasty_core::{
    Result,
    driver::{Capability, Driver},
    schema::{db::Migration, diff},
};
use tokio::sync::Mutex;
use turso::sync::{Builder, Database};

use crate::{TursoBase, error::classify_turso_error};

pub use crate::Connection;

/// Opt-in flags for Turso's experimental features. Each field mirrors a
/// `turso::Builder::experimental_*` method and is applied in
/// [`BuilderOptions::apply`] when the driver constructs a fresh
/// [`turso::sync::Builder`] at connection time.
#[derive(Debug, Default, Clone)]
struct BuilderOptions {
    remote_url: Option<String>,
    index_method: bool,
}

impl BuilderOptions {
    fn apply(&self, mut b: Builder) -> Builder {
        if let Some(remote_url) = &self.remote_url {
            b = b.with_remote_url(remote_url)
        }
        if self.index_method {
            b = b.experimental_index_method(true);
        }
        b
    }
}

///
#[derive(Clone)]
pub struct TursoSync {
    base: TursoBase,
    options: BuilderOptions,
    database: Arc<Mutex<Option<Database>>>,
}

impl TursoSync {
    ///
    pub fn new(url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            base: TursoBase::from_url(url)?,
            options: BuilderOptions::default(),
            database: Arc::new(Mutex::new(None)),
        })
    }

    /// Create an in-memory Turso database.
    pub fn in_memory() -> Self {
        TursoBase::in_memory().into()
    }

    /// Open a Turso database at the specified file path.
    pub fn file<P: AsRef<Path>>(path: P) -> Self {
        TursoBase::file(path).into()
    }

    /// Set remote_url for HTTP requests.
    pub fn with_remote_url(mut self, remote_url: impl Into<String>) -> Self {
        self.options.remote_url = Some(remote_url.into());
        self
    }

    /// Push local changes to the remote.
    pub async fn push(&self) -> Result<()> {
        self.database()
            .await?
            .push()
            .await
            .map_err(classify_turso_error)
    }

    /// Pull remote changes; returns true if any changes were applied.
    pub async fn pull(&self) -> Result<bool> {
        self.database()
            .await?
            .pull()
            .await
            .map_err(classify_turso_error)
    }

    /// Enable Turso's experimental index methods. Mirrors
    /// `turso:sync::Builder::experimental_index_method`.
    pub fn experimental_index_method(mut self, on: bool) -> Self {
        self.options.index_method = on;
        self
    }

    async fn database(&self) -> Result<Database> {
        let mut slot = self.database.lock().await;
        if let Some(db) = slot.as_ref() {
            return Ok(db.clone());
        }
        let builder = self
            .options
            .apply(Builder::new_remote(self.base.path_str()));
        let db = builder.build().await.map_err(classify_turso_error)?;
        *slot = Some(db.clone());
        Ok(db)
    }
}

impl From<TursoBase> for TursoSync {
    fn from(base: TursoBase) -> Self {
        Self {
            base,
            options: BuilderOptions::default(),
            database: Arc::new(Mutex::new(None)),
        }
    }
}

impl fmt::Debug for TursoSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TursoSync")
            .field("path", &self.base.path)
            .field("concurrent_writes", &self.base.concurrent_writes)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Driver for TursoSync {
    fn url(&self) -> Cow<'_, str> {
        self.base.url()
    }

    fn capability(&self) -> &'static Capability {
        TursoBase::capability()
    }

    async fn connect(&self) -> Result<Box<dyn toasty_core::Connection>> {
        let database = self.database().await?;
        let conn = database.connect().await.map_err(classify_turso_error)?;
        self.base.connect(conn).await
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        TursoBase::generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> Result<()> {
        TursoBase::reset_db(&self.database, &self.base.path).await
    }
}
