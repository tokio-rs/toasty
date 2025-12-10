use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Eq,
    Ne,
    Ge,
    Gt,
    Le,
    Lt,
    IsA,
}

impl BinaryOp {
    pub fn is_eq(self) -> bool {
        matches!(self, Self::Eq)
    }

    pub fn is_ne(self) -> bool {
        matches!(self, Self::Ne)
    }

    pub fn is_a(self) -> bool {
        matches!(self, Self::IsA)
    }

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
    /// - `IsA` → `None`
    pub fn negate(self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Ne),
            Self::Ne => Some(Self::Eq),
            Self::Lt => Some(Self::Ge),
            Self::Ge => Some(Self::Lt),
            Self::Gt => Some(Self::Le),
            Self::Le => Some(Self::Gt),
            Self::IsA => None,
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
            Self::IsA => Self::IsA,
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
            BinaryOp::IsA => "is a".fmt(f),
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
