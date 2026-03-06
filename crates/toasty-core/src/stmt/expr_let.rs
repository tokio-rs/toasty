use super::Expr;

/// A scoped binding expression with one or more bindings.
///
/// Evaluates each binding in order, pushes all results into a new scope, then
/// evaluates `body` in that scope. The body references binding `i` via
/// `Arg(position=i, nesting=0)`.
///
/// `ExprLet` is transient scaffolding used during lowering. It is always
/// inlined away (by substituting the bindings into the body) before the
/// planner sees the expression tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprLet {
    /// Expressions whose results are bound as `arg(0)`, `arg(1)`, etc. in a
    /// new scope.
    pub bindings: Vec<Expr>,

    /// The body expression evaluated in the new scope.
    pub body: Box<Expr>,
}

impl From<ExprLet> for Expr {
    fn from(value: ExprLet) -> Self {
        Self::Let(value)
    }
}
