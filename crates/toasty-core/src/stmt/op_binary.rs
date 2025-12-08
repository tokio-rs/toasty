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
    Add,
    Sub,
    Mul,
    Div,
    Mod,
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

    /// Returns true if this is a comparison operator.
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Ge | Self::Gt | Self::Le | Self::Lt | Self::IsA
        )
    }

    /// Returns true if this is an arithmetic operator.
    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod
        )
    }

    pub fn reverse(&mut self) {
        match *self {
            Self::Eq => {}
            _ => todo!(),
        }
    }

    /// Returns the logical negation of this operator, if one exists.
    ///
    /// Only comparison operators can be negated:
    ///
    /// - `=` → `!=`
    /// - `!=` → `=`
    /// - `<` → `>=`
    /// - `>=` → `<`
    /// - `>` → `<=`
    /// - `<=` → `>`
    ///
    /// Returns `None` for `IsA` and arithmetic operators.
    pub fn negate(self) -> Option<Self> {
        match self {
            Self::Eq => Some(Self::Ne),
            Self::Ne => Some(Self::Eq),
            Self::Lt => Some(Self::Ge),
            Self::Ge => Some(Self::Lt),
            Self::Gt => Some(Self::Le),
            Self::Le => Some(Self::Gt),
            Self::IsA | Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Mod => None,
        }
    }

    /// Returns the operator that represents an equivalent expression when the
    /// operands are commuted (swapped) or `None` if the operator cannot be
    /// commuted.
    ///
    /// For comparisons, `5 < x` becomes `x > 5`, so `Lt.commute()` returns
    /// `Some(Gt)`. Symmetric operators like `Eq`, `Ne`, `Add`, and `Mul` return
    /// themselves.
    ///
    /// Non-commutative operators (`Sub`, `Div`, `Mod`) return `None` since
    /// there is no equivalent operator for commuted operands.
    pub fn commute(self) -> Option<Self> {
        match self {
            // Symmetric comparison operators
            Self::Eq => Some(Self::Eq),
            Self::Ne => Some(Self::Ne),
            Self::IsA => Some(Self::IsA),
            // Asymmetric comparison operators
            Self::Ge => Some(Self::Le),
            Self::Gt => Some(Self::Lt),
            Self::Le => Some(Self::Ge),
            Self::Lt => Some(Self::Gt),
            // Commutative arithmetic operators
            Self::Add => Some(Self::Add),
            Self::Mul => Some(Self::Mul),
            // Non-commutative arithmetic operators
            Self::Sub | Self::Div | Self::Mod => None,
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
            BinaryOp::Add => "+".fmt(f),
            BinaryOp::Sub => "-".fmt(f),
            BinaryOp::Mul => "*".fmt(f),
            BinaryOp::Div => "/".fmt(f),
            BinaryOp::Mod => "%".fmt(f),
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
