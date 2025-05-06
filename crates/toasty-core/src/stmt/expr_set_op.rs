use super::*;

#[derive(Debug, Clone)]
pub struct ExprSetOp {
    pub op: SetOp,
    pub operands: Vec<ExprSet>,
}

impl ExprSetOp {
    pub fn is_union(&self) -> bool {
        matches!(self.op, SetOp::Union)
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        for operand in &mut self.operands {
            operand.substitute_ref(input);
        }
    }
}
