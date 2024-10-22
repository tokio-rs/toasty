use super::*;

#[derive(Debug, Clone)]
pub enum ExprList<'stmt> {
    /// A list of expressions
    Expr(Vec<Expr<'stmt>>),

    /// A list of values
    Value(Vec<Value<'stmt>>),

    /// A placeholder, to be populated later
    Placeholder(ExprPlaceholder),
}
