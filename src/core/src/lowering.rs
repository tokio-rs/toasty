use crate::{
    schema::{ColumnId, IndexId, TableId},
    stmt,
};

/// Describes how to map a model to a table
#[derive(Debug, PartialEq)]
pub struct Lowering {
    pub table: TableId,

    /// Table columns used to represent the model.
    pub columns: Vec<ColumnId>,

    pub model_to_table: stmt::ExprRecord<'static>,

    /// How to map the model's primary key to the table's primary key
    pub model_pk_to_table: stmt::Expr<'static>,

    /// How to map table records to model records
    pub table_to_model: stmt::ExprRecord<'static>,
}

/// Describes how to lower a model index to a table index
#[derive(Debug, PartialEq)]
pub struct IndexLowering {
    /// Table index this matches to
    pub index: IndexId,
}

#[derive(Debug, PartialEq)]
pub struct ModelFieldTo {}

impl Lowering {}
