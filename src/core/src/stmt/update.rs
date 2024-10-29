use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Update<'stmt> {
    /// What to update
    pub target: UpdateTarget,

    /// Assignments
    pub assignments: Assignments<'stmt>,

    /// Which entries to update
    pub filter: Option<Expr<'stmt>>,

    /// A condition that must be satisfied in order for the update to apply.
    pub condition: Option<Expr<'stmt>>,

    /// Optionally return data from the update
    pub returning: Option<Returning<'stmt>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateTarget {
    /// Update a model
    Model(ModelId),

    /// Update a table
    Table(TableWithJoins),
}

impl<'stmt> Update<'stmt> {}

impl UpdateTarget {
    pub fn as_model_id(&self) -> ModelId {
        match self {
            UpdateTarget::Model(model_id) => *model_id,
            _ => todo!(),
        }
    }

    pub fn as_table(&self) -> &TableWithJoins {
        match self {
            UpdateTarget::Table(table) => table,
            _ => todo!(),
        }
    }
}

impl<'stmt> From<Update<'stmt>> for Statement<'stmt> {
    fn from(src: Update<'stmt>) -> Statement<'stmt> {
        Statement::Update(src)
    }
}

impl<'stmt> Node<'stmt> for Update<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_update(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_update(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_update_mut(self);
    }
}
