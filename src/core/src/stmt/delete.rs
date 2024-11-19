use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Delete {
    /// Source of data to delete from
    pub from: Source,

    /// WHERE
    pub filter: Expr,

    /// Optionally, return something
    pub returning: Option<Returning>,
}

impl Delete {
    pub fn selection(&self) -> Query {
        stmt::Query::filter(self.from.as_model_id(), self.filter.clone())
    }
}

impl From<Delete> for Statement {
    fn from(src: Delete) -> Statement {
        Statement::Delete(src)
    }
}

impl Node for Delete {
    fn map<V: Map>(&self, visit: &mut V) -> Self {
        visit.map_stmt_delete(self)
    }

    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_delete(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_delete_mut(self);
    }
}
