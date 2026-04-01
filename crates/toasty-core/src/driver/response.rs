use crate::{Result, stmt};

/// The result of a database operation.
///
/// Every database operation produces an `ExecResponse` containing [`Rows`],
/// which may be a row count, a single value, or a stream of result rows.
/// Paginated queries may also include cursors for fetching subsequent pages.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::ExecResponse;
///
/// // Create a count response (e.g., from a DELETE that affected 3 rows)
/// let resp = ExecResponse::count(3);
/// assert_eq!(resp.values.into_count(), 3);
/// ```
#[derive(Debug)]
pub struct ExecResponse {
    /// The result values (rows, count, or stream).
    pub values: Rows,
    /// Cursor to the next page (if paginated and more data exists).
    pub next_cursor: Option<stmt::Value>,
    /// Cursor to the previous page (if backward pagination is supported).
    pub prev_cursor: Option<stmt::Value>,
}

/// The payload of an [`ExecResponse`].
///
/// Operations that modify rows typically return [`Count`](Self::Count).
/// Queries return either a single [`Value`](Self::Value) or a
/// [`Stream`](Self::Stream) of rows.
#[derive(Debug)]
pub enum Rows {
    /// Number of rows affected by the operation (e.g., rows deleted or updated).
    Count(u64),

    /// A single value result.
    Value(stmt::Value),

    /// A stream of result rows, consumed asynchronously.
    Stream(stmt::ValueStream),
}

impl ExecResponse {
    /// Creates a response indicating that `count` rows were affected.
    pub fn count(count: u64) -> Self {
        Self {
            values: Rows::Count(count),
            next_cursor: None,
            prev_cursor: None,
        }
    }

    /// Creates a response wrapping a stream of values.
    pub fn value_stream(values: impl Into<stmt::ValueStream>) -> Self {
        Self {
            values: Rows::value_stream(values),
            next_cursor: None,
            prev_cursor: None,
        }
    }

    /// Creates a response with an empty value stream (no rows).
    pub fn empty_value_stream() -> Self {
        Self {
            values: Rows::Stream(stmt::ValueStream::default()),
            next_cursor: None,
            prev_cursor: None,
        }
    }

    /// Create a response from rows with no pagination cursors.
    pub fn from_rows(rows: Rows) -> Self {
        Self {
            values: rows,
            next_cursor: None,
            prev_cursor: None,
        }
    }
}

impl Rows {
    /// Wraps the given values as a [`Stream`](Self::Stream).
    pub fn value_stream(values: impl Into<stmt::ValueStream>) -> Self {
        Self::Stream(values.into())
    }

    /// Returns `true` if this is a [`Count`](Self::Count) variant.
    pub fn is_count(&self) -> bool {
        matches!(self, Self::Count(_))
    }

    /// If this is a [`Stream`](Self::Stream), collects all values and converts
    /// it to a [`Value`](Self::Value) containing a [`Value::List`](stmt::Value::List).
    /// Other variants are left unchanged.
    pub async fn buffer(&mut self) -> Result<()> {
        if matches!(self, Rows::Stream(_)) {
            let Rows::Stream(stream) = std::mem::replace(self, Rows::Count(0)) else {
                unreachable!()
            };
            *self = Rows::Value(stmt::Value::List(stream.collect().await?));
        }
        Ok(())
    }

    /// Creates a duplicate of this `Rows` value.
    ///
    /// For streams, this buffers the stream contents so both the original and
    /// the duplicate can be consumed independently.
    pub async fn dup(&mut self) -> Result<Self> {
        match self {
            Rows::Count(count) => Ok(Rows::Count(*count)),
            Rows::Value(value) => Ok(Rows::Value(value.clone())),
            Rows::Stream(values) => Ok(Rows::Stream(values.dup().await?)),
        }
    }

    /// Attempts to clone this `Rows` value without async buffering.
    ///
    /// Returns `None` if the stream variant cannot be cloned synchronously.
    pub fn try_clone(&self) -> Option<Self> {
        match self {
            Rows::Count(count) => Some(Rows::Count(*count)),
            Rows::Value(value) => Some(Rows::Value(value.clone())),
            Rows::Stream(values) => values.try_clone().map(Rows::Stream),
        }
    }

    /// Consumes this `Rows` and returns the count.
    ///
    /// # Panics
    ///
    /// Panics if this is not a [`Count`](Self::Count) variant.
    #[track_caller]
    pub fn into_count(self) -> u64 {
        match self {
            Rows::Count(count) => count,
            _ => todo!("rows={self:#?}"),
        }
    }

    /// Collects all rows into a single [`Value::List`](stmt::Value::List).
    ///
    /// For [`Stream`](Self::Stream) variants, this consumes the entire stream.
    /// For [`Value`](Self::Value) variants, returns the value directly.
    ///
    /// # Panics
    ///
    /// Panics if this is a [`Count`](Self::Count) variant.
    pub async fn collect_as_value(self) -> Result<stmt::Value> {
        match self {
            Rows::Count(_) => panic!("expected value; actual={self:#?}"),
            Rows::Value(value) => Ok(value),
            Rows::Stream(stream) => Ok(stmt::Value::List(stream.collect().await?)),
        }
    }

    /// Converts this `Rows` into a [`ValueStream`](stmt::ValueStream).
    ///
    /// [`Value::List`](stmt::Value::List) variants are converted into a stream
    /// from the list items.
    ///
    /// # Panics
    ///
    /// Panics if this is a [`Count`](Self::Count) variant.
    pub fn into_value_stream(self) -> stmt::ValueStream {
        match self {
            Rows::Value(stmt::Value::List(items)) => stmt::ValueStream::from_vec(items),
            Rows::Stream(stream) => stream,
            _ => panic!("expected ValueStream; actual={self:#?}"),
        }
    }
}
