use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub stmt: Box<Statement>,
}

impl Expr {
    pub fn stmt(stmt: impl Into<Statement>) -> Expr {
        Expr::Stmt(ExprStmt {
            stmt: Box::new(stmt.into()),
        })
    }
}

impl From<ExprStmt> for Expr {
    fn from(value: ExprStmt) -> Self {
        Expr::Stmt(value)
    }
}

impl From<Statement> for ExprStmt {
    fn from(value: Statement) -> Self {
        ExprStmt { stmt: value.into() }
    }
}

impl From<Insert> for ExprStmt {
    fn from(value: Insert) -> Self {
        ExprStmt {
            stmt: Box::new(Statement::from(value)),
        }
    }
}
