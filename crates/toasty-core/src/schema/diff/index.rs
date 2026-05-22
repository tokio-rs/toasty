use super::Context;
use crate::schema::db;

use hashbrown::{HashMap, HashSet};

/// A single index-level change between two table versions.
///
/// Computed by [`Index::diff`].
pub enum Index<'a> {
    /// A new index was created.
    Create(&'a db::Index),
    /// An existing index was dropped.
    Drop(&'a db::Index),
    /// An index was modified (name, columns, uniqueness, or other property changed).
    Alter {
        /// The index definition before the change.
        previous: &'a db::Index,
        /// The index definition after the change.
        next: &'a db::Index,
    },
}

impl<'a> Index<'a> {
    /// Computes the diff between two index slices.
    ///
    /// Uses [`Context`] to resolve rename hints for both indices and columns.
    /// Indices matched by name (or by rename hint) are compared; unmatched
    /// indices in `previous` become drops, and unmatched indices in `next`
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
    /// assert!(diff::Index::diff(&cx, &[], &[]).is_empty());
    /// ```
    pub fn diff(cx: &Context<'a>, previous: &'a [db::Index], next: &'a [db::Index]) -> Vec<Self> {
        fn has_diff(cx: &Context<'_>, previous: &db::Index, next: &db::Index) -> bool {
            if previous.name != next.name
                || previous.columns.len() != next.columns.len()
                || previous.unique != next.unique
                || previous.primary_key != next.primary_key
            {
                return true;
            }

            for (previous_col, next_col) in previous.columns.iter().zip(next.columns.iter()) {
                if previous_col.op != next_col.op || previous_col.scope != next_col.scope {
                    return true;
                }

                let columns_match =
                    if let Some(renamed_to) = cx.rename_hints().get_column(previous_col.column) {
                        renamed_to == next_col.column
                    } else {
                        let previous_column = cx.previous().column(previous_col.column);
                        let next_column = cx.next().column(next_col.column);
                        previous_column.name == next_column.name
                    };

                if !columns_match {
                    return true;
                }
            }

            false
        }

        let mut changes = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|to| to.id).collect();

        let next_map =
            HashMap::<&str, &'a db::Index>::from_iter(next.iter().map(|to| (to.name.as_str(), to)));

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_index(previous.id) {
                cx.next().index(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                changes.push(Self::Drop(previous));
                continue;
            };

            create_ids.remove(&next.id);

            if has_diff(cx, previous, next) {
                changes.push(Self::Alter { previous, next });
            }
        }

        for index_id in create_ids {
            changes.push(Self::Create(cx.next().index(index_id)));
        }

        changes
    }
}
