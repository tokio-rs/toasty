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

    pub fn reverse(&self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Ne),
            Self::Ne => Some(Self::Eq),
            Self::Ge => Some(Self::Le),
            Self::Gt => Some(Self::Lt),
            Self::Le => Some(Self::Ge),
            Self::Lt => Some(Self::Gt),
            Self::IsA => None,
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
