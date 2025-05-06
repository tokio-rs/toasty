use super::*;

#[derive(Debug, Clone)]
pub struct ExprTy {
    /// The type
    pub ty: Type,

    /// Optionally, if the type is an enum, this references a variant.
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
