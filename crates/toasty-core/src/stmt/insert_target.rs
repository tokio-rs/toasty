use super::{Expr, InsertTable, Query};
use crate::{
    schema::app::ModelId,
    stmt::{ExprSet, Source},
    Schema,
};

#[derive(Debug, Clone, PartialEq)]
pub enum InsertTarget {
    /// Inserting into a scope implies that the inserted value should be
    /// included by the query after insertion. This could be a combination of
    /// setting default field values or validating existing ones.
    Scope(Box<Query>),

    /// Insert a model
    Model(ModelId),

    /// Insert into a table
    Table(InsertTable),
}

impl InsertTarget {
    pub fn is_model(&self) -> bool {
        matches!(self, InsertTarget::Model(..))
    }

    pub fn as_model_unwrap(&self) -> ModelId {
        match self {
            Self::Scope(query) => query.body.as_select_unwrap().source.model_id_unwrap(),
            Self::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self, InsertTarget::Table(..))
    }

    pub fn as_table_unwrap(&self) -> &InsertTable {
        match self {
            Self::Table(table) => table,
            _ => todo!(),
        }
    }

    pub fn add_constraint(&mut self, expr: impl Into<Expr>) {
        let expr = expr.into();
        match self {
            Self::Scope(query) => query.add_filter(expr),
            Self::Model(model_id) => {
                *self = Self::Scope(Box::new(Query::new_select(*model_id, expr)));
            }
            _ => todo!("{self:#?}"),
        }
    }

    pub fn width(&self, schema: &Schema) -> usize {
        match self {
            InsertTarget::Scope(query) => match &query.body {
                ExprSet::Select(select) => match &select.source {
                    Source::Model(source_model) => {
                        schema.app.model(source_model.model).expect_root().fields.len()
                    }
                    _ => todo!("insert_target={self:#?}"),
                },
                _ => todo!("insert_target={self:#?}"),
            },
            InsertTarget::Model(model_id) => schema.app.model(model_id).expect_root().fields.len(),
            InsertTarget::Table(insert_table) => insert_table.columns.len(),
        }
    }
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        Self::Scope(Box::new(value))
    }
}
