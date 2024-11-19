use super::*;

#[derive(Debug, Clone)]
pub struct ExprOr {
    pub operands: Vec<Expr>,
}
