mod capability;
pub use capability::{Capability, StorageTypes};

mod response;
pub use response::{Response, Rows};

pub mod operation;
pub use operation::Operation;

use crate::{async_trait, schema::db::Schema, stmt};

use std::{fmt::Debug, sync::Arc};

#[async_trait]
pub trait Driver: Debug + Send + Sync + 'static {
    /// Describes the driver's capability, which informs the query planner.
    fn capability(&self) -> &Capability;

    /// Register the schema with the driver.
    async fn register_schema(&mut self, schema: &Schema) -> crate::Result<()>;

    /// Execute a database operation
    async fn exec(&self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response>;

    /// TODO: this will probably go away
    async fn reset_db(&self, _schema: &Schema) -> crate::Result<()> {
        unimplemented!()
    }
}
