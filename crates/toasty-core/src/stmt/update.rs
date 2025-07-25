use super::*;

#[derive(Debug, Clone)]
pub struct Update {
    /// What to update
    pub target: UpdateTarget,

    /// Assignments
    pub assignments: Assignments,

    /// Which entries to update
    pub filter: Option<Expr>,

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

#[derive(Debug, Clone)]
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
        stmt::Query::filter(
            self.target.as_model_id(),
            self.filter.as_ref().unwrap().clone(),
        )
    }
}

impl UpdateTarget {
    #[track_caller]
    pub fn as_model_id(&self) -> ModelId {
        match self {
            Self::Model(model_id) => *model_id,
            Self::Query(query) => query.body.as_select().source.as_model_id(),
            _ => todo!("not a model"),
        }
    }

    pub fn table(table: impl Into<TableId>) -> Self {
        Self::Table(table.into())
    }

    #[track_caller]
    pub fn as_table(&self) -> TableId {
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
