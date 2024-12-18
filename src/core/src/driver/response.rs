use crate::stmt::ValueStream;

#[derive(Debug)]
pub struct Response {
    pub rows: Rows,
}

#[derive(Debug)]
pub enum Rows {
    /// Number of rows impacted by the operation
    Count(usize),

    /// Operation result, as a stream of rows
    Values(ValueStream),
}

impl Response {
    pub fn from_count(count: usize) -> Response {
        Response {
            rows: Rows::Count(count),
        }
    }

    pub fn from_value_stream(values: ValueStream) -> Response {
        Response {
            rows: Rows::Values(values),
        }
    }

    pub fn empty_value_stream() -> Response {
        Response {
            rows: Rows::Values(ValueStream::new()),
        }
    }
}

impl Rows {
    pub fn is_count(&self) -> bool {
        matches!(self, Rows::Count(_))
    }

    pub fn into_values(self) -> ValueStream {
        match self {
            Rows::Values(values) => values,
            _ => todo!(),
        }
    }
}
