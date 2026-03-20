use super::Insert;
use crate::schema::Model;

/// Convert a value into an [`Insert`] statement.
///
/// Generated create-builders implement this trait so that they can be passed
/// anywhere an insert is expected (e.g., association [`insert`](Association::insert)
/// calls or batch operations).
///
/// The associated type [`Model`](IntoInsert::Model) identifies which model the
/// insert targets.
pub trait IntoInsert {
    /// The model this insert targets.
    type Model: Model;

    /// Consume `self` and produce the [`Insert`] statement.
    fn into_insert(self) -> Insert<Self::Model>;
}
