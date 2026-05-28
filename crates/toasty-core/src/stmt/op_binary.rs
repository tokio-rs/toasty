use std::fmt;

/// A binary operator: comparison or arithmetic.
///
/// Used by [`ExprBinaryOp`](super::ExprBinaryOp) to specify the operation
/// applied between two expressions.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::BinaryOp;
///
/// let op = BinaryOp::Eq;
/// assert!(op.is_eq());
/// assert_eq!(op.to_string(), "=");
///
/// // Negation
/// assert_eq!(op.negate(), Some(BinaryOp::Ne));
///
/// // Commutation (swapping operands)
/// assert_eq!(BinaryOp::Lt.commute(), Some(BinaryOp::Gt));
/// assert_eq!(BinaryOp::Add.commute(), Some(BinaryOp::Add));
/// assert_eq!(BinaryOp::Sub.commute(), None);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Equality (`=`).
    Eq,
    /// Inequality (`!=`).
    Ne,
    /// Greater than or equal (`>=`).
    Ge,
    /// Greater than (`>`).
    Gt,
    /// Less than or equal (`<=`).
    Le,
    /// Less than (`<`).
    Lt,
    /// Arithmetic addition (`+`).
    Add,
    /// Arithmetic subtraction (`-`).
    Sub,
}

impl BinaryOp {
    /// Returns `true` if this is the equality operator.
    pub fn is_eq(self) -> bool {
        matches!(self, Self::Eq)
    }

    /// Returns `true` if this is the inequality operator.
    pub fn is_ne(self) -> bool {
        matches!(self, Self::Ne)
    }

    /// Returns `true` if this is an arithmetic operator (`+`, `-`).
    pub fn is_arithmetic(self) -> bool {
        matches!(self, Self::Add | Self::Sub)
    }

    /// Returns the logical negation of this operator, if one exists.
    ///
    /// Only comparison operators have a logical negation; arithmetic
    /// operators return `None`.
    ///
    /// - `=` → `!=`
    /// - `!=` → `=`
    /// - `<` → `>=`
    /// - `>=` → `<`
    /// - `>` → `<=`
    /// - `<=` → `>`
    pub fn negate(self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Ne),
            Self::Ne => Some(Self::Eq),
            Self::Lt => Some(Self::Ge),
            Self::Ge => Some(Self::Lt),
            Self::Gt => Some(Self::Le),
            Self::Le => Some(Self::Gt),
            Self::Add | Self::Sub => None,
        }
    }

    /// Returns the operator that gives an equivalent result when the operands
    /// are swapped, or `None` if the operator is not commutative.
    ///
    /// For example, `5 < x` becomes `x > 5`, so `Lt.commute()` returns
    /// `Some(Gt)`. Symmetric operators (`Eq`, `Ne`, `Add`) return themselves.
    /// `Sub` is not commutative (`a - b ≠ b - a`) and returns `None`.
    pub fn commute(self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Eq),
            Self::Ne => Some(Self::Ne),
            Self::Ge => Some(Self::Le),
            Self::Gt => Some(Self::Lt),
            Self::Le => Some(Self::Ge),
            Self::Lt => Some(Self::Gt),
            Self::Add => Some(Self::Add),
            Self::Sub => None,
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Eq => "=".fmt(f),
            BinaryOp::Ne => "!=".fmt(f),
            BinaryOp::Ge => ">=".fmt(f),
            BinaryOp::Gt => ">".fmt(f),
            BinaryOp::Le => "<=".fmt(f),
            BinaryOp::Lt => "<".fmt(f),
            BinaryOp::Add => "+".fmt(f),
            BinaryOp::Sub => "-".fmt(f),
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
