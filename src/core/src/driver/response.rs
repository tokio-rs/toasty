use crate::stmt::ValueStream;

#[derive(Debug)]
pub struct Response<'stmt> {
    pub rows: Rows<'stmt>,
}

#[derive(Debug)]
pub enum Rows<'stmt> {
    /// Number of rows impacted by the operation
    Count(usize),

    /// Operation result, as a stream of rows
    Values(ValueStream<'stmt>),
}

impl<'stmt> Response<'stmt> {
    pub fn from_count(count: usize) -> Response<'stmt> {
        Response {
            rows: Rows::Count(count),
        }
    }

    pub fn from_value_stream(values: ValueStream<'stmt>) -> Response<'stmt> {
        Response {
            rows: Rows::Values(values),
        }
    }
}

impl<'stmt> Rows<'stmt> {
    pub fn is_count(&self) -> bool {
        matches!(self, Rows::Count(_))
    }

    pub fn into_values(self) -> ValueStream<'stmt> {
        match self {
            Rows::Values(values) => values,
            _ => todo!(),
        }
    }
}
