use crate::Error;
use toasty_core::stmt;

/// Load an instance of a type from a [`Value`][stmt::Value].
///
/// The value is expected to be a `Value::Record` containing the type's fields.
/// This trait is implemented by both root models and any other types that can
/// be deserialized from the database value representation.
pub trait Load: Sized {
    fn load(value: stmt::Value) -> Result<Self, Error>;
}
