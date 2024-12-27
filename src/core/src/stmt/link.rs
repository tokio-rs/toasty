use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// The source of the association
    pub source: Query,

    /// The association field to link
    pub field: FieldId,

    /// Models to associate
    pub target: Query,
}

impl From<Link> for Statement {
    fn from(value: Link) -> Self {
        Statement::Link(value)
    }
}

impl Node for Link {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_link(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_link_mut(self);
    }
}
