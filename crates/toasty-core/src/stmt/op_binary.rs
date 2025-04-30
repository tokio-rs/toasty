use std::fmt;

#[derive(Copy, Clone, PartialEq)]
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
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;

        match self {
            Eq => "=".fmt(f),
            Ne => "!=".fmt(f),
            Ge => ">=".fmt(f),
            Gt => ">".fmt(f),
            Le => "<=".fmt(f),
            Lt => "<".fmt(f),
            IsA => "is a".fmt(f),
        }
    }
}

impl fmt::Debug for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
