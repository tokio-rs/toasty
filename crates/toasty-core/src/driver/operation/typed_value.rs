use crate::{schema::db, stmt};

/// A bind parameter value paired with its database storage type.
///
/// Produced by the engine's parameter extraction pass, which infers
/// the `db::Type` for each value from the statement's column context.
/// SQL drivers use the storage type to pick the correct wire format
/// (e.g., a native enum OID instead of `TEXT` for PostgreSQL).
#[derive(Debug, Clone)]
pub struct TypedValue {
    /// The parameter value.
    pub value: stmt::Value,
    /// The database storage type of the target column.
    pub ty: db::Type,
}
