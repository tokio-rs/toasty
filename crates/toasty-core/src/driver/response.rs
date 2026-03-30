use crate::{Result, stmt};

/// The result returned by [`Connection::exec`](super::Connection::exec).
///
/// Every database operation produces a `Response` containing [`Rows`], which
/// may be a row count, a single value, or a stream of result rows.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::Response;
///
/// // Create a count response (e.g., from a DELETE that affected 3 rows)
/// let resp = Response::count(3);
/// assert_eq!(resp.rows.into_count(), 3);
/// ```
#[derive(Debug)]
pub struct Response {
    /// The rows produced by the operation.
    pub rows: Rows,
    /// Opaque cursor for pagination. Driver-specific format.
    pub cursor: Option<stmt::Value>,
}

/// The payload of a [`Response`].
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

impl Response {
    /// Creates a response indicating that `count` rows were affected.
    pub fn count(count: u64) -> Self {
        Self {
            rows: Rows::Count(count),
            cursor: None,
        }
    }

    /// Creates a response wrapping a stream of values.
    pub fn value_stream(values: impl Into<stmt::ValueStream>) -> Self {
        Self {
            rows: Rows::value_stream(values),
            cursor: None,
        }
    }

    /// Creates a response with an empty value stream (no rows).
    pub fn empty_value_stream() -> Self {
        Self {
            rows: Rows::Stream(stmt::ValueStream::default()),
            cursor: None,
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
