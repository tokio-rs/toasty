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

// TODO: This is a hack to aid in the transition from schema code gen to proc
// macro. This should be removed once the proc macro is implemented.
impl<T: Model> Model for Option<T> {
    const ID: ModelId = T::ID;

    fn load(row: stmt::ValueRecord) -> Result<Self, Error> {
        Ok(Some(T::load(row)?))
    }

    fn schema() -> app::Model {
        todo!()
    }
}
