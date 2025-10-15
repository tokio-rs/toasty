use crate::stmt::Query;

#[derive(Debug, Clone, PartialEq)]
pub struct TableDerived {
    pub subquery: Box<Query>,
}
