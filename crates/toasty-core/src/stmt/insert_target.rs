use super::{Expr, InsertTable, Query};
use crate::schema::app::ModelId;

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

    #[track_caller]
    pub fn expect_model(&self) -> ModelId {
        match self {
            Self::Scope(query) => query.body.expect_select().source.model_id_unwrap(),
            Self::Model(model_id) => *model_id,
            _ => panic!("expected InsertTarget::Model; actual={self:#?}"),
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self, InsertTarget::Table(..))
    }

    pub fn as_table(&self) -> Option<&InsertTable> {
        match self {
            Self::Table(table) => Some(table),
            _ => None,
        }
    }

    #[track_caller]
    pub fn expect_table(&self) -> &InsertTable {
        self.as_table()
            .unwrap_or_else(|| panic!("expected InsertTarget::Table; actual={self:#?}"))
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
}

impl From<Query> for InsertTarget {
    fn from(value: Query) -> Self {
        Self::Scope(Box::new(value))
    }
}
