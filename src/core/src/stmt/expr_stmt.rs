use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt<'stmt> {
    pub stmt: Box<Statement<'stmt>>,
}

impl<'stmt> ExprStmt<'stmt> {
    pub fn new<T>(stmt: T) -> ExprStmt<'stmt>
    where
        T: Into<Statement<'stmt>>,
    {
        ExprStmt {
            stmt: Box::new(stmt.into()),
        }
    }
}

impl<'stmt> From<ExprStmt<'stmt>> for Expr<'stmt> {
    fn from(value: ExprStmt<'stmt>) -> Self {
        Expr::Stmt(value)
    }
}

impl<'stmt> From<Statement<'stmt>> for ExprStmt<'stmt> {
    fn from(value: Statement<'stmt>) -> Self {
        ExprStmt { stmt: value.into() }
    }
}

impl<'stmt> From<Insert<'stmt>> for ExprStmt<'stmt> {
    fn from(value: Insert<'stmt>) -> Self {
        ExprStmt {
            stmt: Box::new(Statement::from(value)),
        }
    }
}
