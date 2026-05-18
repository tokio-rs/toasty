use super::Context;
use crate::schema::db::Index;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two index lists.
///
/// Computed by [`Indices::from`] and dereferences to `Vec<IndicesItem>` for
/// iteration.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{Schema, diff};
///
/// let previous = Schema::default();
/// let next = Schema::default();
/// let hints = diff::RenameHints::new();
/// let cx = diff::Context::new(&previous, &next, &hints);
/// let d = diff::Indices::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Indices<'a> {
    items: Vec<IndicesItem<'a>>,
}

impl<'a> Indices<'a> {
    /// Computes the diff between two index slices.
    ///
    /// Uses [`Context`] to resolve rename hints for both indices and columns.
    /// Indices matched by name (or by rename hint) are compared; unmatched
    /// indices in `previous` become drops, and unmatched indices in `next`
    /// become creates.
    pub fn from(cx: &Context<'a>, previous: &'a [Index], next: &'a [Index]) -> Self {
        fn has_diff(cx: &Context<'_>, previous: &Index, next: &Index) -> bool {
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

        let mut items = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|to| to.id).collect();

        let next_map =
            HashMap::<&str, &'a Index>::from_iter(next.iter().map(|to| (to.name.as_str(), to)));

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_index(previous.id) {
                cx.next().index(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                items.push(IndicesItem::DropIndex(previous));
                continue;
            };

            create_ids.remove(&next.id);

            if has_diff(cx, previous, next) {
                items.push(IndicesItem::AlterIndex { previous, next });
            }
        }

        for index_id in create_ids {
            items.push(IndicesItem::CreateIndex(cx.next().index(index_id)));
        }

        Self { items }
    }

    /// Returns `true` if there are no index changes.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for Indices<'a> {
    type Target = Vec<IndicesItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two index lists.
pub enum IndicesItem<'a> {
    /// A new index was created.
    CreateIndex(&'a Index),
    /// An existing index was dropped.
    DropIndex(&'a Index),
    /// An index was modified (name, columns, uniqueness, or other property changed).
    AlterIndex {
        /// The index definition before the change.
        previous: &'a Index,
        /// The index definition after the change.
        next: &'a Index,
    },
}
