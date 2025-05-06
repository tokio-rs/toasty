use super::*;

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub stmt: Box<Statement>,
}

impl Expr {
    pub fn stmt(stmt: impl Into<Statement>) -> Self {
        Self::Stmt(ExprStmt {
            stmt: Box::new(stmt.into()),
        })
    }
}

impl From<ExprStmt> for Expr {
    fn from(value: ExprStmt) -> Self {
        Self::Stmt(value)
    }
}

impl From<Statement> for ExprStmt {
    fn from(value: Statement) -> Self {
        Self { stmt: value.into() }
    }
}

impl From<Insert> for ExprStmt {
    fn from(value: Insert) -> Self {
        Self {
            stmt: Box::new(Statement::from(value)),
        }
    }
}
