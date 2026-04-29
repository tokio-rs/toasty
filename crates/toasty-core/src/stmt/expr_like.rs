use super::Expr;

/// A SQL `LIKE` pattern-match expression: `expr LIKE pattern`.
///
/// Returns `true` if `expr` matches `pattern`. The user is responsible for
/// including any `%` or `_` wildcard characters in `pattern`.
///
/// # Examples
///
/// ```text
/// name LIKE 'Al%'   // true if name starts with "Al"
/// name LIKE '%son'  // true if name ends with "son"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprLike {
    /// The attribute to test.
    pub expr: Box<Expr>,

    /// The LIKE pattern (including any % or _ wildcards).
    pub pattern: Box<Expr>,

    /// Optional escape character. When `Some(c)`, occurrences of `c` in the
    /// pattern make the following `%`, `_`, or `c` match literally, and the
    /// serializer emits an `ESCAPE 'c'` clause.
    pub escape: Option<char>,
}

impl Expr {
    /// Creates a `expr LIKE pattern` expression with no escape character.
    pub fn like(expr: impl Into<Self>, pattern: impl Into<Self>) -> Self {
        ExprLike {
            expr: Box::new(expr.into()),
            pattern: Box::new(pattern.into()),
            escape: None,
        }
        .into()
    }
}

impl From<ExprLike> for Expr {
    fn from(value: ExprLike) -> Self {
        Self::Like(value)
    }
}
