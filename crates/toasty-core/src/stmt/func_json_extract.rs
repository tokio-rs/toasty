use super::{Expr, ExprFunc, Type};

/// Extracts the value at a key path from a document-stored value.
///
/// Produced when a query filters on a field inside a `#[document]` embed:
/// `User::FIELDS.preferences().theme()` lowers to a `JsonExtract` over the
/// `preferences` document column with `path = ["theme"]`. The SQL serializer
/// renders it per dialect — `col->'a'->>'b'` on PostgreSQL, `json_extract(col,
/// '$.a.b')` on SQLite / MySQL.
#[derive(Clone, Debug, PartialEq)]
pub struct FuncJsonExtract {
    /// The document value to extract from — typically a column reference.
    pub base: Box<Expr>,

    /// The key path from the document root to the extracted field, in order.
    pub path: Vec<String>,

    /// The type of the extracted value (the leaf field's type).
    pub ty: Type,
}

impl From<FuncJsonExtract> for ExprFunc {
    fn from(value: FuncJsonExtract) -> Self {
        Self::JsonExtract(value)
    }
}

impl From<FuncJsonExtract> for Expr {
    fn from(value: FuncJsonExtract) -> Self {
        Self::Func(value.into())
    }
}
