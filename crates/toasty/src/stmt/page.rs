use std::ops::Deref;

use super::{List, Paginate, Query};
use crate::schema::Load;
use crate::{Executor, Result};
use toasty_core::stmt;

/// A page of results from a cursor-based paginated query.
///
/// Obtained by calling [`Paginate::exec`]. The
/// page contains up to `per_page` items and optional cursors for fetching the
/// next or previous page.
///
/// `Page<M>` dereferences to `[M]`, so it can be used anywhere a slice is
/// expected.
///
/// # Navigation
///
/// Call [`next`](Page::next) or [`prev`](Page::prev) to fetch adjacent pages.
/// Use [`has_next`](Page::has_next) and [`has_prev`](Page::has_prev) to check
/// availability without fetching.
#[derive(Debug)]
pub struct Page<M> {
    /// Items in this page.
    pub items: Vec<M>,

    /// Base query (without cursors/offsets).
    query: Query<List<M>>,

    /// Cursor for fetching next page (opaque value from driver)
    pub next_cursor: Option<stmt::Value>,

    /// Cursor for fetching previous page (opaque value from driver)
    pub prev_cursor: Option<stmt::Value>,
}

impl<M> Page<M> {
    pub(crate) fn new(
        items: Vec<M>,
        query: Query<List<M>>,
        next_cursor: Option<stmt::Value>,
        prev_cursor: Option<stmt::Value>,
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

impl<M: Load> Page<M> {
    /// Fetches the next page of results.
    ///
    /// Returns `None` if there are no more pages available. Uses the cursor from
    /// the last item in the current page to fetch the next set of results.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use toasty::Db;
    /// # async fn example<T: toasty::schema::Model>(db: &mut Db, page: toasty::stmt::Page<T>) -> toasty::Result<()> {
    /// if let Some(next_page) = page.next(db).await? {
    ///     println!("Found {} items in next page", next_page.items.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next(&self, executor: &mut dyn Executor) -> Result<Option<Page<M::Output>>> {
        match &self.next_cursor {
            Some(cursor) => Ok(Some(
                Paginate::from(self.query.clone())
                    .after(cursor.clone())
                    .exec(executor)
                    .await?,
            )),
            None => Ok(None),
        }
    }

    /// Fetches the previous page of results.
    ///
    /// Returns `None` if there are no previous pages available. Uses the cursor from
    /// the first item in the current page to fetch the previous set of results.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use toasty::Db;
    /// # async fn example<T: toasty::schema::Model>(db: &mut Db, page: toasty::stmt::Page<T>) -> toasty::Result<()> {
    /// if let Some(prev_page) = page.prev(db).await? {
    ///     println!("Found {} items in previous page", prev_page.items.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn prev(&self, executor: &mut dyn Executor) -> Result<Option<Page<M::Output>>> {
        match &self.prev_cursor {
            Some(cursor) => Ok(Some(
                Paginate::from(self.query.clone())
                    .before(cursor.clone())
                    .exec(executor)
                    .await?,
            )),
            None => Ok(None),
        }
    }
}

// Allow using pages like a regular slice for ergonomics.
impl<M> Deref for Page<M> {
    type Target = [M];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
