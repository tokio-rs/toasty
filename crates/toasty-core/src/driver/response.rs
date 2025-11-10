use crate::{stmt::ValueStream, Result};

#[derive(Debug)]
pub struct Response {
    pub rows: Rows,
}

#[derive(Debug)]
pub enum Rows {
    /// Number of rows impacted by the operation
    Count(u64),

    /// Operation result, as a stream of rows
    Values(ValueStream),
}

impl Response {
    pub fn count(count: u64) -> Self {
        Self {
            rows: Rows::Count(count),
        }
    }

    pub fn value_stream(values: impl Into<ValueStream>) -> Self {
        Self {
            rows: Rows::value_stream(values),
        }
    }

    pub fn empty_value_stream() -> Self {
        Self {
            rows: Rows::Values(ValueStream::default()),
        }
    }
}

impl Rows {
    pub fn value_stream(values: impl Into<ValueStream>) -> Self {
        Self::Values(values.into())
    }

    pub fn is_count(&self) -> bool {
        matches!(self, Self::Count(_))
    }

    pub fn is_values(&self) -> bool {
        matches!(self, Self::Values(_))
    }

    pub async fn dup(&mut self) -> Result<Self> {
        match self {
            Rows::Count(count) => Ok(Rows::Count(*count)),
            Rows::Values(values) => Ok(Rows::Values(values.dup().await?)),
        }
    }

    pub fn try_clone(&self) -> Option<Self> {
        match self {
            Rows::Count(count) => Some(Rows::Count(*count)),
            Rows::Values(values) => values.try_clone().map(Rows::Values),
        }
    }

    #[track_caller]
    pub fn into_count(self) -> u64 {
        match self {
            Rows::Count(count) => count,
            _ => todo!("rows={self:#?}"),
        }
    }

    #[track_caller]
    pub fn into_values(self) -> ValueStream {
        match self {
            Self::Values(values) => values,
            _ => todo!("rows={self:#?}"),
        }
    }
}
