use super::*;

#[derive(Debug, Clone)]
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
    pub fn as_model(&self) -> ModelId {
        match self {
            Self::Scope(query) => query.body.as_select().source.as_model_id(),
            Self::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }

    pub fn as_table(&self) -> &InsertTable {
        match self {
            Self::Table(table) => table,
            _ => todo!(),
        }
    }

    pub fn constrain(&mut self, expr: impl Into<Expr>) {
        match self {
            Self::Scope(query) => query.and(expr),
            Self::Model(model_id) => {
                *self = Self::Scope(Box::new(Query::filter(*model_id, expr)));
            }
            _ => todo!("{self:#?}"),
        }
    }
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        Self::Scope(Box::new(value))
    }
}
