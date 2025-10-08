use super::{ExprSet, SetOp};

#[derive(Debug, Clone)]
pub struct ExprSetOp {
    pub op: SetOp,
    pub operands: Vec<ExprSet>,
}

impl ExprSetOp {
    pub fn is_union(&self) -> bool {
        matches!(self.op, SetOp::Union)
    }
}
