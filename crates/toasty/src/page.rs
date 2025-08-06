use crate::stmt::Select;
use crate::{Db, Model, Result};
use toasty_core::stmt;

/// A page of results from a paginated query.
#[derive(Debug)]
pub struct Page<M> {
    /// Items in this page
    pub items: Vec<M>,

    /// Base query (without cursors/offsets)
    query: Select<M>,

    /// Cursor for fetching next page (derived from last item)
    pub next_cursor: Option<stmt::Expr>,

    /// Cursor for fetching previous page (derived from first item)
    pub prev_cursor: Option<stmt::Expr>,
}

impl<M: Model> Page<M> {
    pub(crate) fn new(
        items: Vec<M>,
        query: Select<M>,
        next_cursor: Option<stmt::Expr>,
        prev_cursor: Option<stmt::Expr>,
    ) -> Self {
        Self {
            items,
            query,
            next_cursor,
            prev_cursor,
        }
    }

    /// Returns true if there is a next page available
    pub fn has_next(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Returns true if there is a previous page available
    pub fn has_prev(&self) -> bool {
        self.prev_cursor.is_some()
    }
}
