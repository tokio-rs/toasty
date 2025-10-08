use crate::stmt::Query;

#[derive(Debug, Clone)]
pub struct TableDerived {
    pub subquery: Box<Query>,
}
