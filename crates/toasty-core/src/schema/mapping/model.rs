use super::*;

#[derive(Debug, Clone)]
pub struct Model {
    /// Model identiier
    pub id: ModelId,

    /// Table that the model maps to
    pub table: TableId,

    /// Table columns used to represent the model.
    pub columns: Vec<ColumnId>,

    /// Primitive fields map to column fields
    pub fields: Vec<Option<Field>>,

    /// How to map a model expression to a table expression
    pub model_to_table: stmt::ExprRecord,

    /// How to map the model's primary key to the table's primary key
    pub model_pk_to_table: stmt::Expr,

    /// How to map a table record to a model record
    pub table_to_model: stmt::ExprRecord,
}
