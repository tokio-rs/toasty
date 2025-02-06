use crate::Error;
use toasty_core::{schema::app::ModelId, stmt};

pub trait Model: Sized {
    /// Unique identifier for this model within the schema.
    ///
    /// Identifiers are *not* unique across schemas.
    const ID: ModelId;

    /// Model key type
    type Key;

    /// Load an instance of the model, populating fields using the given row.
    fn load(row: stmt::ValueRecord) -> Result<Self, Error>;
}

pub trait Relation<'a> {
    type ManyField;
    type OneField;
}
