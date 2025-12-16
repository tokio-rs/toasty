mod capability;
pub use capability::{Capability, StorageTypes};

mod response;
pub use response::{Response, Rows};

pub mod operation;
pub use operation::Operation;

use crate::schema::db::Schema;

use std::{fmt::Debug, future::Future, sync::Arc};

pub trait Driver: Debug + Send + Sync + 'static {
    type Connection: Connection;
    fn connect(&self) -> crate::Result<Self::Connection>;
}

pub trait Connection: Debug + Send + Sync + 'static {
    /// Describes the driver's capability, which informs the query planner.
    fn capability(&self) -> &'static Capability;

    /// Execute a database operation
    fn exec(
        &self,
        schema: &Arc<Schema>,
        plan: Operation,
    ) -> impl Future<Output = crate::Result<Response>>;

    /// TODO: this will probably go away
    fn reset_db(&self, _schema: &Schema) -> impl Future<Output = crate::Result<()>> {
        unimplemented!();
        async { Ok(()) }
    }
}
