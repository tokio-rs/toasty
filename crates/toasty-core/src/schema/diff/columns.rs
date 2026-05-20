use super::Context;
use crate::schema::db::Column;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two column lists.
///
/// Computed by [`Columns::from`] and dereferences to `Vec<ColumnsItem>` for
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
/// let d = diff::Columns::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Columns<'a> {
    items: Vec<ColumnsItem<'a>>,
}

impl<'a> Columns<'a> {
    /// Computes the diff between two column slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Columns matched by name (or
    /// by rename hint) are compared field-by-field; unmatched columns in
    /// `previous` become drops, and unmatched columns in `next` become adds.
    pub fn from(cx: &Context<'a>, previous: &'a [Column], next: &'a [Column]) -> Self {
        fn has_diff(previous: &Column, next: &Column) -> bool {
            previous.name != next.name
                || previous.storage_ty != next.storage_ty
                || previous.nullable != next.nullable
                || previous.primary_key != next.primary_key
                || previous.auto_increment != next.auto_increment
                || previous.versionable != next.versionable
        }

        let mut items = vec![];
        let mut add_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map =
            HashMap::<&str, &'a Column>::from_iter(next.iter().map(|to| (to.name.as_str(), to)));

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_column(previous.id) {
                cx.next().column(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                items.push(ColumnsItem::DropColumn(previous));
                continue;
            };

            add_ids.remove(&next.id);

            if has_diff(previous, next) {
                items.push(ColumnsItem::AlterColumn { previous, next });
            }
        }

        for column_id in add_ids {
            items.push(ColumnsItem::AddColumn(cx.next().column(column_id)));
        }

        Self { items }
    }

    /// Returns `true` if there are no column changes.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for Columns<'a> {
    type Target = Vec<ColumnsItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two column lists.
pub enum ColumnsItem<'a> {
    /// A new column was added.
    AddColumn(&'a Column),
    /// An existing column was removed.
    DropColumn(&'a Column),
    /// A column was modified (name, type, nullability, or other property changed).
    AlterColumn {
        /// The column definition before the change.
        previous: &'a Column,
        /// The column definition after the change.
        next: &'a Column,
    },
}
