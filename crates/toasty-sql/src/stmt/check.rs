use super::Ident;
use toasty_core::stmt::Expr;

/// A `CHECK` constraint, usable at both column and table level.
///
/// Mirrors sqlparser's `CheckConstraint` struct.
#[derive(Debug, Clone)]
pub struct CheckConstraint {
    /// Optional constraint name (`CONSTRAINT <name> CHECK ...`).
    pub name: Option<Ident>,
    /// The boolean expression the CHECK constraint enforces.
    pub expr: Box<Expr>,
}
