use super::*;

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateTarget {
    /// Update a model
    Model(ModelId),

    /// Update a table
    Table(TableWithJoins),
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
    pub fn as_model_id(&self) -> ModelId {
        match self {
            UpdateTarget::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }

    pub fn table(table: impl Into<TableId>) -> UpdateTarget {
        UpdateTarget::Table(TableWithJoins {
            table: table.into(),
        })
    }

    pub fn as_table(&self) -> &TableWithJoins {
        match self {
            UpdateTarget::Table(table) => table,
            _ => todo!(),
        }
    }
}

impl From<Update> for Statement {
    fn from(src: Update) -> Statement {
        Statement::Update(src)
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
