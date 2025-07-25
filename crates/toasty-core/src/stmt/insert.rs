use super::*;

#[derive(Debug, Clone)]
pub struct Insert {
    /// Where to insert the values
    pub target: InsertTarget,

    /// Source of values to insert
    pub source: Query,

    /// Optionally return data from the insertion
    pub returning: Option<Returning>,
}

impl Insert {
    pub fn merge(&mut self, other: Self) {
        match (&self.target, &other.target) {
            (InsertTarget::Model(a), InsertTarget::Model(b)) if a == b => {}
            _ => todo!("handle this case"),
        }

        match (&mut self.source.body, other.source.body) {
            (stmt::ExprSet::Values(self_values), stmt::ExprSet::Values(other_values)) => {
                for expr in other_values.rows {
                    self_values.rows.push(expr);
                }
            }
            (self_source, other) => todo!("self={:#?}; other={:#?}", self_source, other),
        }
    }
}

impl From<Insert> for Statement {
    fn from(src: Insert) -> Self {
        Self::Insert(src)
    }
}

impl Node for Insert {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_insert(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_insert_mut(self);
    }
}
