use std::ops::Deref;

use crate::stmt::{Paginate, Select};
use crate::Result;
use crate::{Db, Model};
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

    /// Fetches the next page of results.
    ///
    /// Returns `None` if there are no more pages available. Uses the cursor from
    /// the last item in the current page to fetch the next set of results.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use toasty::Db;
    /// # async fn example<T: toasty::Model>(db: &Db, page: toasty::Page<T>) -> toasty::Result<()> {
    /// if let Some(next_page) = page.next(db).await? {
    ///     println!("Found {} items in next page", next_page.items.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next(&self, db: &Db) -> Result<Option<Page<M>>> {
        match &self.next_cursor {
            Some(cursor) => Ok(Some(
                Paginate::from(self.query.clone())
                    .after(cursor.clone())
                    .collect(db)
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
    /// # async fn example<T: toasty::Model>(db: &Db, page: toasty::Page<T>) -> toasty::Result<()> {
    /// if let Some(prev_page) = page.prev(db).await? {
    ///     println!("Found {} items in previous page", prev_page.items.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn prev(&self, db: &Db) -> Result<Option<Page<M>>> {
        match &self.prev_cursor {
            Some(cursor) => Ok(Some(
                Paginate::from(self.query.clone())
                    .before(cursor.clone())
                    .collect(db)
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
