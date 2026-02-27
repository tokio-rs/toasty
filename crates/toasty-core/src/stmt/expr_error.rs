use super::Expr;

/// An expression representing an unreachable branch.
///
/// `Expr::Error` marks code paths that should never execute at runtime. The
/// primary use is the else branch of `ExprMatch` on enum discriminants: all
/// valid discriminants are covered by explicit arms, so the else branch is
/// semantically unreachable. If it IS reached (e.g., due to data corruption or
/// a schema mismatch), evaluation fails with the contained message.
///
/// # Simplifier semantics
///
/// Because Error is unreachable, simplification rules treat it as an opaque
/// value — no special propagation is needed. Existing rules handle it
/// naturally:
///
/// - `false AND (Error == x)` → `false` (short-circuit on `false`)
/// - `Record([disc, Error]) == Record([I64(1), "alice"])` decomposes into
///   `disc == I64(1) AND Error == "alice"`, and if `disc == I64(1)`
///   contradicts a guard like `disc != I64(1)`, the whole AND folds to
///   `false`.
///
/// In all well-formed cases, the guard constraints around Error cause the
/// branch to be pruned without requiring Error-specific rules.
///
/// # Type inference
///
/// `Expr::Error` infers as `Type::Unknown`. `TypeUnion::insert` skips
/// `Unknown`, so an Error branch doesn't widen inferred type unions.
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
