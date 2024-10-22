use super::*;

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Eq,
    Divide,
    Ge,
    Gt,
    Minus,
    Multiply,
    Ne,
    Le,
    Lt,
    Plus,
}

impl BinaryOp {
    pub(crate) fn from_stmt(stmt: stmt::BinaryOp) -> BinaryOp {
        match stmt {
            stmt::BinaryOp::Eq => BinaryOp::Eq,
            stmt::BinaryOp::Ne => BinaryOp::Ne,
            stmt::BinaryOp::Gt => BinaryOp::Gt,
            stmt::BinaryOp::Ge => BinaryOp::Ge,
            stmt::BinaryOp::Le => BinaryOp::Le,
            stmt::BinaryOp::Lt => BinaryOp::Lt,
            _ => todo!("binary_op = {:#?}", stmt),
        }
    }
}
