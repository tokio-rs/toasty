use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Unlink<'stmt> {
    /// The source of the association
    pub source: Query<'stmt>,

    /// The association field to unlink
    pub field: FieldId,

    /// Models to disassociate
    pub target: Query<'stmt>,
}

impl<'stmt> From<Unlink<'stmt>> for Statement<'stmt> {
    fn from(value: Unlink<'stmt>) -> Self {
        Statement::Unlink(value)
    }
}

impl<'stmt> Node<'stmt> for Unlink<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_unlink(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_unlink(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_unlink_mut(self);
    }
}
