use super::Field;
use crate::{
    schema::{
        app::ModelId,
        db::{ColumnId, TableId},
    },
    stmt,
};

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
    pub table_to_model: TableToModel,
}

#[derive(Debug, Default, Clone)]
pub struct TableToModel {
    expr: stmt::ExprRecord,
}

impl TableToModel {
    pub fn new(expr: stmt::ExprRecord) -> TableToModel {
        TableToModel { expr }
    }

    pub fn lower_returning_model(&self) -> stmt::Expr {
        self.expr.clone().into()
    }

    pub fn lower_expr_reference(&self, nesting: usize, index: usize) -> stmt::Expr {
        let mut expr = self.expr[index].clone();
        let n = nesting;

        if n > 0 {
            stmt::visit_mut::for_each_expr_mut(&mut expr, |expr| {
                if let stmt::Expr::Reference(stmt::ExprReference::Column { nesting, .. }) = expr {
                    *nesting = n;
                }
            });
        }

        expr
    }
}
