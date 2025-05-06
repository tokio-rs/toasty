use super::*;

use crate::schema::db;

#[derive(Debug, Clone)]
pub struct FieldPrimitive {
    /// The field's primitive type
    pub ty: stmt::Type,

    /// The database storage type of the field.
    ///
    /// This is specified as a hint.
    pub storage_ty: Option<db::Type>,
}
