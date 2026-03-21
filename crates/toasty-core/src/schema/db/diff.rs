use std::collections::HashMap;

use crate::schema::db::{ColumnId, IndexId, Schema, TableId};

/// Hints that tell the diff algorithm which schema items were renamed.
///
/// Without rename hints, a renamed table/column/index appears as a drop
/// followed by a create. Adding a hint maps the old ID to the new ID so
/// the diff produces an alter instead.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{RenameHints, TableId};
///
/// let mut hints = RenameHints::new();
/// hints.add_table_hint(TableId(0), TableId(1));
/// assert_eq!(hints.get_table(TableId(0)), Some(TableId(1)));
/// assert_eq!(hints.get_table(TableId(2)), None);
/// ```
#[derive(Default)]
pub struct RenameHints {
    tables: HashMap<TableId, TableId>,
    columns: HashMap<ColumnId, ColumnId>,
    indices: HashMap<IndexId, IndexId>,
}

impl RenameHints {
    /// Creates an empty set of rename hints.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that the table previously identified by `from` is now identified by `to`.
    pub fn add_table_hint(&mut self, from: TableId, to: TableId) {
        self.tables.insert(from, to);
    }

    /// Records that the column previously identified by `from` is now identified by `to`.
    pub fn add_column_hint(&mut self, from: ColumnId, to: ColumnId) {
        self.columns.insert(from, to);
    }

    /// Records that the index previously identified by `from` is now identified by `to`.
    pub fn add_index_hint(&mut self, from: IndexId, to: IndexId) {
        self.indices.insert(from, to);
    }

    /// Returns the new [`TableId`] if a rename hint exists for `from`.
    pub fn get_table(&self, from: TableId) -> Option<TableId> {
        self.tables.get(&from).copied()
    }

    /// Returns the new [`ColumnId`] if a rename hint exists for `from`.
    pub fn get_column(&self, from: ColumnId) -> Option<ColumnId> {
        self.columns.get(&from).copied()
    }

    /// Returns the new [`IndexId`] if a rename hint exists for `from`.
    pub fn get_index(&self, from: IndexId) -> Option<IndexId> {
        self.indices.get(&from).copied()
    }
}

/// Shared context passed to all diff computations.
///
/// Holds references to both the previous and next [`Schema`] versions and
/// the [`RenameHints`] that guide rename detection.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{DiffContext, RenameHints, Schema};
///
/// let previous = Schema::default();
/// let next = Schema::default();
/// let hints = RenameHints::new();
/// let cx = DiffContext::new(&previous, &next, &hints);
/// assert!(cx.previous().tables.is_empty());
/// ```
pub struct DiffContext<'a> {
    previous: &'a Schema,
    next: &'a Schema,

    rename_hints: &'a RenameHints,
}

impl<'a> DiffContext<'a> {
    /// Creates a new diff context from the previous schema, the next schema,
    /// and the rename hints that map old IDs to new IDs.
    pub fn new(previous: &'a Schema, next: &'a Schema, rename_hints: &'a RenameHints) -> Self {
        Self {
            previous,
            next,
            rename_hints,
        }
    }

    /// Returns the rename hints for this diff.
    pub fn rename_hints(&self) -> &'a RenameHints {
        self.rename_hints
    }

    /// Returns the schema before the change.
    pub fn previous(&self) -> &'a Schema {
        self.previous
    }

    /// Returns the schema after the change.
    pub fn next(&self) -> &'a Schema {
        self.next
    }
}
