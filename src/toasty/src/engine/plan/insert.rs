use super::*;

#[derive(Debug)]
pub(crate) struct Insert {
    /// Where to get the input from
    pub input: Vec<Input>,

    /// If the output is needed, store it in this variable
    pub output: Option<InsertOutput>,

    /// The insert statement
    pub stmt: stmt::Insert,
}

#[derive(Debug)]
pub(crate) struct InsertOutput {
    /// Where to store the output
    pub var: VarId,

    /// Expected argument type.
    pub arg_ty: stmt::Type,

    /// How to project it before storing
    pub project: eval::Expr,
}

impl From<Insert> for Action {
    fn from(src: Insert) -> Action {
        Action::Insert(src)
    }
}
