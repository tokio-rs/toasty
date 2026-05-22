use super::{Context, RenameHints, Table, Type};
use crate::schema::db::Schema as DbSchema;

/// The top-level diff between two database schemas.
///
/// Contains a [`Tables`] describing created, dropped, and altered tables.
/// Constructed via [`Schema::from`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::{db::Schema as DbSchema, diff};
///
/// let previous = DbSchema::default();
/// let next = DbSchema::default();
/// let hints = diff::RenameHints::new();
/// let d = diff::Schema::from(&previous, &next, &hints);
/// assert!(d.is_empty());
/// ```
pub struct Schema<'a> {
    previous: &'a DbSchema,
    next: &'a DbSchema,
    tables: Vec<Table<'a>>,
}

impl<'a> Schema<'a> {
    /// Computes the diff between two schemas, using the provided rename hints.
    pub fn from(from: &'a DbSchema, to: &'a DbSchema, rename_hints: &'a RenameHints) -> Self {
        let cx = Context::new(from, to, rename_hints);
        Self {
            previous: from,
            next: to,
            tables: Table::diff(&cx, &from.tables, &to.tables),
        }
    }

    /// Computes the enum type diff between the two schemas.
    pub fn types(&self) -> Vec<Type<'a>> {
        Type::diff(self.previous, self.next)
    }

    /// Returns the table-level diff.
    pub fn tables(&self) -> &[Table<'a>] {
        &self.tables
    }

    /// Returns `true` if no tables were created, dropped, or altered.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// Returns the schema before the change.
    pub fn previous(&self) -> &'a DbSchema {
        self.previous
    }

    /// Returns the schema after the change.
    pub fn next(&self) -> &'a DbSchema {
        self.next
    }
}
