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

    /// When used in a returning context for an include, this indicates whether
    /// the parent field is nullable (i.e., `Option<T>`). Used during lowering
    /// to set the `nullable` flag on the resulting `Arg::Sub` in the HIR so
    /// that NestedMerge can produce the correct Option encoding.
    pub nullable: bool,
}

impl Expr {
    pub fn stmt(stmt: impl Into<Statement>) -> Self {
        Self::Stmt(ExprStmt {
            stmt: Box::new(stmt.into()),
            nullable: false,
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
        Self {
            stmt: value.into(),
            nullable: false,
        }
    }
}

impl From<Insert> for ExprStmt {
    fn from(value: Insert) -> Self {
        Self {
            stmt: Box::new(Statement::from(value)),
            nullable: false,
        }
    }
}

impl From<Insert> for Expr {
    fn from(value: Insert) -> Self {
        Self::Stmt(value.into())
    }
}
