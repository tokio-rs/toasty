use super::{Expr, Insert, Statement};

/// A statement used as an expression.
///
/// Wraps a statement (such as a subquery) so it can be used in expression
/// contexts.
///
/// # Examples
///
/// ```text
/// (SELECT max(age) FROM users)   // subquery as expression
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    /// The wrapped statement.
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

impl From<Insert> for Expr {
    fn from(value: Insert) -> Self {
        Self::Stmt(value.into())
    }
}
