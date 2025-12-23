use std::fmt;

/// Unary operators for expressions.
// TODO: consider pulling logical negation (`Not`) into this enum.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Arithmetic negation (`-x`)
    Neg,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => "-".fmt(f),
        }
    }
}

impl fmt::Debug for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
