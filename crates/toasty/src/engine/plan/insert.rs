use super::*;

#[derive(Debug)]
pub(crate) struct Insert {
    /// Where to get the input from
    pub input: Option<Input>,

    /// If the output is needed, store it in this variable
    pub output: Option<Output>,

    /// The insert statement
    pub stmt: stmt::Insert,
}

impl From<Insert> for Action {
    fn from(src: Insert) -> Self {
        Self::Insert(src)
    }
}
