use super::{Assignments, Expr, Node, Query, Returning, Statement, Visit, VisitMut};
use crate::{
    schema::{app::ModelId, db::TableId},
    stmt::{self, Filter},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// What to update
    pub target: UpdateTarget,

    /// Assignments
    pub assignments: Assignments,

    /// Which entries to update
    pub filter: Filter,

    /// A condition that must be satisfied in order for the update to apply.
    pub condition: Option<Expr>,

    /// Optionally return data from the update
    pub returning: Option<Returning>,
}

impl Statement {
    pub fn is_update(&self) -> bool {
        matches!(self, Self::Update(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateTarget {
    /// The query must return a "model" for it to be updated.
    Query(Box<Query>),

    /// Update a model
    Model(ModelId),

    /// Update a table
    Table(TableId),
}

impl Update {
    pub fn selection(&self) -> Query {
        stmt::Query::new_select(self.target.model_id_unwrap(), self.filter.clone())
    }
}

impl UpdateTarget {
    pub fn model_id(&self) -> Option<ModelId> {
        match self {
            Self::Model(model_id) => Some(*model_id),
            Self::Query(query) => query
                .body
                .as_select()
                .and_then(|select| select.source.model_id()),
            _ => None,
        }
    }

    #[track_caller]
    pub fn model_id_unwrap(&self) -> ModelId {
        match self {
            Self::Model(model_id) => *model_id,
            Self::Query(query) => query.body.as_select_unwrap().source.model_id_unwrap(),
            _ => todo!("not a model"),
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self, UpdateTarget::Table(..))
    }

    pub fn table(table: impl Into<TableId>) -> Self {
        Self::Table(table.into())
    }

    #[track_caller]
    pub fn as_table_unwrap(&self) -> TableId {
        match self {
            Self::Table(table) => *table,
            _ => todo!(),
        }
    }
}

impl From<Update> for Statement {
    fn from(src: Update) -> Self {
        Self::Update(src)
    }
}

impl Node for Update {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_update(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_update_mut(self);
    }
}
