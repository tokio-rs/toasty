pub mod capability;
pub use capability::Capability;

pub mod operation;
pub use operation::Operation;

use crate::{
    async_trait, eval,
    stmt::{self, ValueStream},
    Schema,
};

use std::fmt::Debug;

#[async_trait]
pub trait Driver: Debug + Send + Sync + 'static {
    /// Describes the driver's capability, which informs the query planner.
    fn capability(&self) -> &Capability;

    /// Register the schema with the driver.
    async fn register_schema(&mut self, schema: &Schema) -> crate::Result<()>;

    /// Execute a database operation
    async fn exec<'stmt>(
        &self,
        schema: &Schema,
        plan: Operation<'stmt>,
    ) -> crate::Result<ValueStream<'stmt>>;

    /// TODO: this will probably go away
    async fn reset_db(&self, _schema: &Schema) -> crate::Result<()> {
        unimplemented!()
    }
}
