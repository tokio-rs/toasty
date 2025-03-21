use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum InsertTarget {
    /// Inserting into a scope implies that the inserted value should be
    /// included by the query after insertion. This could be a combination of
    /// setting default field values or validating existing ones.
    Scope(Query),

    /// Insert a model
    Model(ModelId),

    /// Insert into a table
    Table(InsertTable),
}

impl InsertTarget {
    pub fn as_model(&self) -> ModelId {
        match self {
            InsertTarget::Scope(query) => query.body.as_select().source.as_model_id(),
            InsertTarget::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }

    pub fn as_table(&self) -> &InsertTable {
        match self {
            InsertTarget::Table(table) => table,
            _ => todo!(),
        }
    }

    pub fn constrain(&mut self, expr: impl Into<Expr>) {
        match self {
            InsertTarget::Scope(query) => query.and(expr),
            InsertTarget::Model(model_id) => {
                *self = InsertTarget::Scope(Query::filter(*model_id, expr));
            }
            _ => todo!("{self:#?}"),
        }
    }
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        InsertTarget::Scope(value)
    }
}
