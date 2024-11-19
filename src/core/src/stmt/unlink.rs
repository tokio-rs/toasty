use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Unlink {
    /// The source of the association
    pub source: Query,

    /// The association field to unlink
    pub field: FieldId,

    /// Models to disassociate
    pub target: Query,
}

impl From<Unlink> for Statement {
    fn from(value: Unlink) -> Self {
        Statement::Unlink(value)
    }
}

impl Node for Unlink {
    fn map<V: Map>(&self, visit: &mut V) -> Self {
        visit.map_stmt_unlink(self)
    }

    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_unlink(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_unlink_mut(self);
    }
}
