use super::{Node, Query, Returning, Source, Statement, Visit, VisitMut};
use crate::stmt::{self, Filter};

#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    /// Source of data to delete from
    pub from: Source,

    /// WHERE
    pub filter: Filter,

    /// Optionally, return something
    pub returning: Option<Returning>,
}

impl Delete {
    pub fn selection(&self) -> Query {
        stmt::Query::new_select(self.from.model_id_unwrap(), self.filter.clone())
    }
}

impl From<Delete> for Statement {
    fn from(src: Delete) -> Self {
        Self::Delete(src)
    }
}

impl Node for Delete {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_delete(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_delete_mut(self);
    }
}
