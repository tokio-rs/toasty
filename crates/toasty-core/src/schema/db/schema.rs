use super::{
    Column, ColumnId, DiffContext, Index, IndexId, RenameHints, Table, TableId, TablesDiff,
};

/// The complete database-level schema: a collection of tables.
///
/// Provides indexed access to tables, columns, and indices by their IDs.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::Schema;
///
/// let schema = Schema::default();
/// assert!(schema.tables.is_empty());
/// ```
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Schema {
    /// All tables in this schema.
    pub tables: Vec<Table>,
}

impl Schema {
    /// Returns the column identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table or column index is out of bounds.
    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        let id = id.into();
        self.table(id.table)
            .columns
            .get(id.index)
            .expect("invalid column ID")
    }

    /// Returns a mutable reference to the column identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table or column index is out of bounds.
    pub fn column_mut(&mut self, id: impl Into<ColumnId>) -> &mut Column {
        let id = id.into();
        self.table_mut(id.table)
            .columns
            .get_mut(id.index)
            .expect("invalid column ID")
    }

    /// Returns the index identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table or index offset is out of bounds.
    // NOTE: this is unlikely to confuse users given the context.
    #[allow(clippy::should_implement_trait)]
    pub fn index(&self, id: IndexId) -> &Index {
        self.table(id.table)
            .indices
            .get(id.index)
            .expect("invalid index ID")
    }

    /// Returns a mutable reference to the index identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table or index offset is out of bounds.
    // NOTE: this is unlikely to confuse users given the context.
    #[allow(clippy::should_implement_trait)]
    pub fn index_mut(&mut self, id: IndexId) -> &mut Index {
        self.table_mut(id.table)
            .indices
            .get_mut(id.index)
            .expect("invalid index ID")
    }

    /// Returns the table identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table index is out of bounds.
    pub fn table(&self, id: impl Into<TableId>) -> &Table {
        self.tables.get(id.into().0).expect("invalid table ID")
    }

    /// Returns a mutable reference to the table identified by `id`.
    ///
    /// # Panics
    ///
    /// Panics if the table index is out of bounds.
    pub fn table_mut(&mut self, id: impl Into<TableId>) -> &mut Table {
        self.tables.get_mut(id.into().0).expect("invalid table ID")
    }
}

/// The top-level diff between two database schemas.
///
/// Contains a [`TablesDiff`] describing created, dropped, and altered tables.
/// Constructed via [`SchemaDiff::from`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{SchemaDiff, RenameHints, Schema};
///
/// let previous = Schema::default();
/// let next = Schema::default();
/// let hints = RenameHints::new();
/// let diff = SchemaDiff::from(&previous, &next, &hints);
/// assert!(diff.is_empty());
/// ```
pub struct SchemaDiff<'a> {
    previous: &'a Schema,
    next: &'a Schema,
    tables: TablesDiff<'a>,
}

impl<'a> SchemaDiff<'a> {
    /// Computes the diff between two schemas, using the provided rename hints.
    pub fn from(from: &'a Schema, to: &'a Schema, rename_hints: &'a RenameHints) -> Self {
        let cx = &DiffContext::new(from, to, rename_hints);
        Self {
            previous: from,
            next: to,
            tables: TablesDiff::from(cx, &from.tables, &to.tables),
        }
    }

    /// Returns the table-level diff.
    pub fn tables(&self) -> &TablesDiff<'a> {
        &self.tables
    }

    /// Returns `true` if no tables were created, dropped, or altered.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
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
