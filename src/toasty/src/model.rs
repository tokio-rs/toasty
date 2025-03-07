use crate::Error;
use toasty_core::{
    schema::app::{self, ModelId},
    stmt,
};

pub trait Model: Sized {
    /// Unique identifier for this model within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    const ID: ModelId;

    /// Load an instance of the model, populating fields using the given row.
    fn load(row: stmt::ValueRecord) -> Result<Self, Error>;

    fn schema() -> app::Model {
        todo!()
    }
}
