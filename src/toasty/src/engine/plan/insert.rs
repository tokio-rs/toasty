use super::*;

#[derive(Debug)]
pub(crate) struct Insert<'stmt> {
    /// Where to get the input from
    pub input: Vec<Input<'stmt>>,

    /// If the output is needed, store it in this variable
    pub output: Option<InsertOutput<'stmt>>,

    /// The insert statement
    pub stmt: sql::Insert<'stmt>,
}

#[derive(Debug)]
pub(crate) struct InsertOutput<'stmt> {
    /// Where to store the output
    pub var: VarId,

    /// How to project it before storing
    pub project: eval::Expr<'stmt>,

    /// The type of the returned value
    pub ty: stmt::Type,
}

impl<'stmt> From<Insert<'stmt>> for Action<'stmt> {
    fn from(src: Insert<'stmt>) -> Action<'stmt> {
        Action::Insert(src)
    }
}
