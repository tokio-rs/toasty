use super::Expr;

/// An expression that, when evaluated, produces an error.
///
/// This is used for conditional branches that represent error states. For
/// example, `ExprMatch` may be semantically exhaustive, but because the database
/// may return unexpected values, we still need an else branch. That else branch
/// is an `Expr::Error` — it should never be reached at runtime, but if it is,
/// evaluation fails with the contained message.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprError {
    /// The error message to surface if this expression is evaluated.
    pub message: String,
}

impl Expr {
    pub fn error(message: impl Into<String>) -> Self {
        ExprError {
            message: message.into(),
        }
        .into()
    }
}

impl From<ExprError> for Expr {
    fn from(value: ExprError) -> Self {
        Self::Error(value)
    }
}
