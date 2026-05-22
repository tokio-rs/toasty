use super::{Column, Context, Index};
use crate::schema::db;

use hashbrown::{HashMap, HashSet};

/// A single table-level change between two schema versions.
///
/// Computed by [`Table::diff`].
pub enum Table<'a> {
    /// A new table was created.
    Create(&'a db::Table),
    /// An existing table was dropped.
    Drop(&'a db::Table),
    /// A table was modified (name, columns, or indices changed).
    Alter {
        /// The table definition before the change.
        previous: &'a db::Table,
        /// The table definition after the change.
        next: &'a db::Table,
        /// Column-level changes within this table.
        columns: Vec<Column<'a>>,
        /// Index-level changes within this table.
        indices: Vec<Index<'a>>,
    },
}

impl<'a> Table<'a> {
    /// Computes the diff between two table slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Tables matched by name (or
    /// by rename hint) are compared for column and index changes; unmatched
    /// tables in `previous` become drops, and unmatched tables in `next`
    /// become creates.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toasty_core::schema::{db, diff};
    ///
    /// let previous = db::Schema::default();
    /// let next = db::Schema::default();
    /// let hints = diff::RenameHints::new();
    /// let cx = diff::Context::new(&previous, &next, &hints);
    /// assert!(diff::Table::diff(&cx, &[], &[]).is_empty());
    /// ```
    pub fn diff(cx: &Context<'a>, previous: &'a [db::Table], next: &'a [db::Table]) -> Vec<Self> {
        let mut changes = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map = HashMap::<&str, &'a db::Table>::from_iter(
            next.iter().map(|next| (next.name.as_str(), next)),
        );

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_table(previous.id) {
                cx.next().table(next_id)
            } else if let Some(to) = next_map.get(previous.name.as_str()) {
                to
            } else {
                changes.push(Self::Drop(previous));
                continue;
            };

            create_ids.remove(&next.id);

            let columns = Column::diff(cx, &previous.columns, &next.columns);
            let indices = Index::diff(cx, &previous.indices, &next.indices);
            if previous.name != next.name || !columns.is_empty() || !indices.is_empty() {
                changes.push(Self::Alter {
                    previous,
                    next,
                    columns,
                    indices,
                });
            }
        }

        for table_id in create_ids {
            changes.push(Self::Create(cx.next().table(table_id)));
        }

        changes
    }
}
