use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Link<'stmt> {
    /// The source of the association
    pub source: Query<'stmt>,

    /// The association field to link
    pub field: FieldId,

    /// Models to associate
    pub target: Query<'stmt>,
}

impl<'stmt> From<Link<'stmt>> for Statement<'stmt> {
    fn from(value: Link<'stmt>) -> Self {
        Statement::Link(value)
    }
}

impl<'stmt> Node<'stmt> for Link<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_link(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_link(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_link_mut(self);
    }
}
