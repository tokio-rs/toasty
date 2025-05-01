use crate::stmt::ValueStream;

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
    pub fn from_count(count: u64) -> Self {
        Self {
            rows: Rows::Count(count),
        }
    }

    pub fn from_value_stream(values: ValueStream) -> Self {
        Self {
            rows: Rows::Values(values),
        }
    }

    pub fn empty_value_stream() -> Self {
        Self {
            rows: Rows::Values(ValueStream::default()),
        }
    }
}

impl Rows {
    pub fn is_count(&self) -> bool {
        matches!(self, Self::Count(_))
    }

    pub fn into_values(self) -> ValueStream {
        match self {
            Self::Values(values) => values,
            _ => todo!(),
        }
    }
}
