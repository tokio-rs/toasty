use super::Error;

/// Error when expression evaluation fails.
///
/// This occurs when:
/// - An expression cannot be evaluated in the current context (DEFAULT, WITH clauses)
/// - Required data is missing (unresolved references or arguments)
/// - Type mismatches during evaluation (expected string, got something else)
/// - Expression evaluation is attempted on non-evaluable constructs
///
/// These are runtime evaluation failures, not syntax errors.
#[derive(Debug)]
pub(super) struct ExpressionEvaluationFailed {
    message: Box<str>,
}

impl std::error::Error for ExpressionEvaluationFailed {}

impl core::fmt::Display for ExpressionEvaluationFailed {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "expression evaluation failed: {}", self.message)
    }
}

impl Error {
    /// Creates an expression evaluation failed error.
    ///
    /// This is used when expression evaluation fails at runtime due to:
    /// - Missing context or data
    /// - Type mismatches
    /// - Non-evaluable constructs
    pub fn expression_evaluation_failed(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::ExpressionEvaluationFailed(
            ExpressionEvaluationFailed {
                message: message.into().into(),
            },
        ))
    }

    /// Returns `true` if this error is an expression evaluation failure.
    pub fn is_expression_evaluation_failed(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::ExpressionEvaluationFailed(_))
    }
}
