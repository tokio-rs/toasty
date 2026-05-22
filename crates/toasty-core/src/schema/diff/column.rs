use super::Context;
use crate::schema::db;

use hashbrown::{HashMap, HashSet};

/// A single column-level change between two table versions.
///
/// Computed by [`Column::diff`].
pub enum Column<'a> {
    /// A new column was added.
    Add(&'a db::Column),
    /// An existing column was removed.
    Drop(&'a db::Column),
    /// A column was modified (name, type, nullability, or other property changed).
    Alter {
        /// The column definition before the change.
        previous: &'a db::Column,
        /// The column definition after the change.
        next: &'a db::Column,
    },
}

impl<'a> Column<'a> {
    /// Computes the diff between two column slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Columns matched by name (or
    /// by rename hint) are compared field-by-field; unmatched columns in
    /// `previous` become drops, and unmatched columns in `next` become adds.
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
    /// assert!(diff::Column::diff(&cx, &[], &[]).is_empty());
    /// ```
    pub fn diff(cx: &Context<'a>, previous: &'a [db::Column], next: &'a [db::Column]) -> Vec<Self> {
        fn has_diff(previous: &db::Column, next: &db::Column) -> bool {
            previous.name != next.name
                || previous.storage_ty != next.storage_ty
                || previous.nullable != next.nullable
                || previous.primary_key != next.primary_key
                || previous.auto_increment != next.auto_increment
                || previous.versionable != next.versionable
        }

        let mut changes = vec![];
        let mut add_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map = HashMap::<&str, &'a db::Column>::from_iter(
            next.iter().map(|to| (to.name.as_str(), to)),
        );

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_column(previous.id) {
                cx.next().column(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                changes.push(Self::Drop(previous));
                continue;
            };

            add_ids.remove(&next.id);

            if has_diff(previous, next) {
                changes.push(Self::Alter { previous, next });
            }
        }

        for column_id in add_ids {
            changes.push(Self::Add(cx.next().column(column_id)));
        }

        changes
    }
}
