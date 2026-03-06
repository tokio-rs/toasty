use super::{Expr, Value};

/// A match expression that dispatches on a subject expression.
///
/// Each arm matches the subject against a constant value pattern and evaluates
/// the corresponding expression. `Expr::Match` is never serialized to SQL — it
/// is either evaluated in the engine (for writes) or eliminated by the
/// simplifier before the plan stage (for reads/queries).
#[derive(Debug, Clone, PartialEq)]
pub struct ExprMatch {
    /// The expression to dispatch on.
    pub subject: Box<Expr>,

    /// The match arms, in order.
    pub arms: Vec<MatchArm>,

    /// Fallback expression evaluated when no arm matches.
    pub else_expr: Box<Expr>,
}

/// A single arm in a match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// The constant value pattern this arm matches against.
    pub pattern: Value,

    /// The expression to evaluate when the pattern matches.
    pub expr: Expr,
}

impl Expr {
    /// Creates a `Match` expression that dispatches on `subject`.
    pub fn match_expr(
        subject: impl Into<Self>,
        arms: Vec<MatchArm>,
        else_expr: impl Into<Self>,
    ) -> Self {
        ExprMatch {
            subject: Box::new(subject.into()),
            arms,
            else_expr: Box::new(else_expr.into()),
        }
        .into()
    }
}

impl From<ExprMatch> for Expr {
    fn from(value: ExprMatch) -> Self {
        Self::Match(value)
    }
}
