use super::{Columns, Context, Indices};
use crate::schema::db::Table;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two table lists.
///
/// Computed by [`Tables::from`] and dereferences to `Vec<TablesItem>` for
/// iteration.
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
/// let d = diff::Tables::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Tables<'a> {
    items: Vec<TablesItem<'a>>,
}

impl<'a> Tables<'a> {
    /// Computes the diff between two table slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Tables matched by name (or
    /// by rename hint) are compared for column and index changes; unmatched
    /// tables in `previous` become drops, and unmatched tables in `next`
    /// become creates.
    pub fn from(cx: &Context<'a>, previous: &'a [Table], next: &'a [Table]) -> Self {
        let mut items = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map = HashMap::<&str, &'a Table>::from_iter(
            next.iter().map(|next| (next.name.as_str(), next)),
        );

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_table(previous.id) {
                cx.next().table(next_id)
            } else if let Some(to) = next_map.get(previous.name.as_str()) {
                to
            } else {
                items.push(TablesItem::DropTable(previous));
                continue;
            };

            create_ids.remove(&next.id);

            let columns = Columns::from(cx, &previous.columns, &next.columns);
            let indices = Indices::from(cx, &previous.indices, &next.indices);
            if previous.name != next.name || !columns.is_empty() || !indices.is_empty() {
                items.push(TablesItem::AlterTable {
                    previous,
                    next,
                    columns,
                    indices,
                });
            }
        }

        for table_id in create_ids {
            items.push(TablesItem::CreateTable(cx.next().table(table_id)));
        }

        Self { items }
    }
}

impl<'a> Deref for Tables<'a> {
    type Target = Vec<TablesItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two table lists.
pub enum TablesItem<'a> {
    /// A new table was created.
    CreateTable(&'a Table),
    /// An existing table was dropped.
    DropTable(&'a Table),
    /// A table was modified (name, columns, or indices changed).
    AlterTable {
        /// The table definition before the change.
        previous: &'a Table,
        /// The table definition after the change.
        next: &'a Table,
        /// Column-level changes within this table.
        columns: Columns<'a>,
        /// Index-level changes within this table.
        indices: Indices<'a>,
    },
}
