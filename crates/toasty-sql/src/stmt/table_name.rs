use super::ident::Ident;
use toasty_core::schema::db::TableId;

/// A reference to a table, either by schema ID or by a string identifier.
#[derive(Debug, Clone)]
pub enum TableName {
    /// Reference by schema-assigned table ID.
    TableId(TableId),
    /// Reference by a literal identifier string.
    Ident(Ident),
}

impl From<TableId> for TableName {
    fn from(value: TableId) -> Self {
        TableName::TableId(value)
    }
}

impl From<Ident> for TableName {
    fn from(value: Ident) -> Self {
        TableName::Ident(value)
    }
}

impl<'a> From<&'a str> for TableName {
    fn from(value: &'a str) -> Self {
        TableName::Ident(Ident::from(value))
    }
}
