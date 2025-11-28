use super::{Expr, Type};

/// A type expression.
///
/// Represents a type, optionally with a specific enum variant.
///
/// # Examples
///
/// ```text
/// type(String)       // the String type
/// type(Status, 0)    // variant 0 of the Status enum
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprTy {
    /// The type.
    pub ty: Type,

    /// If the type is an enum, the specific variant index.
    pub variant: Option<usize>,
}

impl ExprTy {
    pub fn new(ty: Type, variant: Option<usize>) -> Self {
        Self { ty, variant }
    }
}

impl From<ExprTy> for Expr {
    fn from(value: ExprTy) -> Self {
        Self::Type(value)
    }
}
