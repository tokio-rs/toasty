use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprSetOp<'stmt> {
    pub op: SetOp,
    pub operands: Vec<ExprSet<'stmt>>,
}

impl<'stmt> ExprSetOp<'stmt> {
    pub fn is_union(&self) -> bool {
        matches!(self.op, SetOp::Union)
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        for operand in &mut self.operands {
            operand.substitute_ref(input);
        }
    }
}
