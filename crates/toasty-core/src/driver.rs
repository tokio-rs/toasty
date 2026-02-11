mod capability;
pub use capability::{Capability, StorageTypes};

mod response;
pub use response::{Response, Rows};

pub mod operation;
pub use operation::Operation;

use crate::{
    async_trait,
    schema::db::{AppliedMigration, Migration, Schema, SchemaDiff},
};

use std::{fmt::Debug, sync::Arc};

#[async_trait]
pub trait Driver: Debug + Send + Sync + 'static {
    /// Describes the driver's capability, which informs the query planner.
    fn capability(&self) -> &'static Capability;

    /// Creates a new connection to the database.
    ///
    /// This method is called by the [`Pool`] whenever a [`Connection`] is requested while none is
    /// available and there is room to create a new [`Connection`].
    async fn connect(&self) -> crate::Result<Box<dyn Connection>>;

    /// Returns the maximum number of simultaneous database connections supported. For example,
    /// this is `Some(1)` for the in-memory SQLite driver which cannot be pooled.
    fn max_connections(&self) -> Option<usize> {
        None
    }

    /// Generates a migration from a [`SchemaDiff`].
    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration;
}

#[async_trait]
pub trait Connection: Debug + Send + 'static {
    /// Execute a database operation
    async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response>;

    /// TODO: this will probably go away
    async fn reset_db(&mut self, _schema: &Schema) -> crate::Result<()> {
        unimplemented!()
    }

    async fn applied_migrations(&mut self) -> crate::Result<Vec<AppliedMigration>>;
    async fn apply_migration(
        &mut self,
        id: u64,
        name: String,
        migration: &Migration,
    ) -> crate::Result<()>;
}
