use crate::Error;
use toasty_core::{schema::app::ModelId, stmt};

pub trait Model: Sized {
    /// Unique identifier for this model within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    const ID: ModelId;

    /// Load an instance of the model, populating fields using the given row.
    fn load(row: stmt::ValueRecord) -> Result<Self, Error>;

    /// Returns the macro-time schema representation for this model.
    ///
    /// This contains unresolved references that will be resolved during
    /// schema registration when all models are known.
    fn schema() -> crate::schema::Model;
}

// TODO: This is a hack to aid in the transition from schema code gen to proc
// macro. This should be removed once the proc macro is implemented.
impl<T: Model> Model for Option<T> {
    const ID: ModelId = T::ID;

    fn load(row: stmt::ValueRecord) -> Result<Self, Error> {
        Ok(Some(T::load(row)?))
    }

    fn schema() -> crate::schema::Model {
        T::schema()
    }
}
