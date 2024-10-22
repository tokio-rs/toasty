use super::*;

#[derive(Debug, Clone)]
pub enum ExprSet<'stmt> {
    Select(Select<'stmt>),
    Values(Values<'stmt>),
}
