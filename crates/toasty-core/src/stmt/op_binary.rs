use std::fmt;

/// A binary comparison operator.
///
/// Used by [`ExprBinaryOp`](super::ExprBinaryOp) to specify the comparison
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
/// assert_eq!(BinaryOp::Lt.commute(), BinaryOp::Gt);
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

    /// Reverses the operator in place (currently only supports `Eq`).
    ///
    /// # Panics
    ///
    /// Panics (via `todo!()`) for operators other than `Eq`.
    pub fn reverse(&mut self) {
        match *self {
            Self::Eq => {}
            _ => todo!(),
        }
    }

    /// Returns the logical negation of this operator, if one exists.
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
        }
    }

    /// Returns the operator that represents an equivalent comparison when the
    /// operands are commuted (swapped).
    ///
    /// For example, `5 < x` becomes `x > 5`, so `Lt.commute()` returns `Gt`.
    /// Symmetric operators like `Eq` and `Ne` return themselves.
    pub fn commute(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Ne => Self::Ne,
            Self::Ge => Self::Le,
            Self::Gt => Self::Lt,
            Self::Le => Self::Ge,
            Self::Lt => Self::Gt,
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
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
