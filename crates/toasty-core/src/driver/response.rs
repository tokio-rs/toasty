use crate::{stmt, Result};

#[derive(Debug)]
pub struct Response {
    pub rows: Rows,
}

#[derive(Debug)]
pub enum Rows {
    /// Number of rows impacted by the operation
    Count(u64),

    /// A single value
    Value(stmt::Value),

    /// Operation result, as a stream of rows
    Stream(stmt::ValueStream),
}

impl Response {
    pub fn count(count: u64) -> Self {
        Self {
            rows: Rows::Count(count),
        }
    }

    pub fn value_stream(values: impl Into<stmt::ValueStream>) -> Self {
        Self {
            rows: Rows::value_stream(values),
        }
    }

    pub fn empty_value_stream() -> Self {
        Self {
            rows: Rows::Stream(stmt::ValueStream::default()),
        }
    }
}

impl Rows {
    pub fn value_stream(values: impl Into<stmt::ValueStream>) -> Self {
        Self::Stream(values.into())
    }

    pub fn is_count(&self) -> bool {
        matches!(self, Self::Count(_))
    }

    pub async fn dup(&mut self) -> Result<Self> {
        match self {
            Rows::Count(count) => Ok(Rows::Count(*count)),
            Rows::Value(value) => Ok(Rows::Value(value.clone())),
            Rows::Stream(values) => Ok(Rows::Stream(values.dup().await?)),
        }
    }

    pub fn try_clone(&self) -> Option<Self> {
        match self {
            Rows::Count(count) => Some(Rows::Count(*count)),
            Rows::Value(value) => Some(Rows::Value(value.clone())),
            Rows::Stream(values) => values.try_clone().map(Rows::Stream),
        }
    }

    #[track_caller]
    pub fn into_count(self) -> u64 {
        match self {
            Rows::Count(count) => count,
            _ => todo!("rows={self:#?}"),
        }
    }

    pub async fn collect_as_value(self) -> Result<stmt::Value> {
        match self {
            Rows::Count(_) => panic!("expected value; actual={self:#?}"),
            Rows::Value(value) => Ok(value),
            Rows::Stream(stream) => Ok(stmt::Value::List(stream.collect().await?)),
        }
    }

    pub fn into_value_stream(self) -> stmt::ValueStream {
        match self {
            Rows::Value(stmt::Value::List(items)) => stmt::ValueStream::from_vec(items),
            Rows::Stream(stream) => stream,
            _ => panic!("expected ValueStream; actual={self:#?}"),
        }
    }
}
