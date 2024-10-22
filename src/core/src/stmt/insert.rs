use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Insert<'stmt> {
    /// The scope in which the record is inserted. This identifies the model
    /// being inserted and is used to populate defaults fields and some level of
    /// validations.
    pub scope: Query<'stmt>,

    /// Expression that evaluates to the values to insert.
    pub values: Expr<'stmt>,

    /// Optionally return data from the insertion
    pub returning: Option<Returning<'stmt>>,
}
impl<'stmt> Insert<'stmt> {
    pub fn merge(&mut self, other: Insert<'stmt>) {
        if self.scope != other.scope {
            todo!("handle this case");
        }

        match (&mut self.values, other.values) {
            (Expr::Record(expr_record), Expr::Record(other)) => {
                for expr in other {
                    expr_record.push(expr);
                }
            }
            (self_values, other) => todo!("self={:#?}; other={:#?}", self_values, other),
        }
    }
}

impl<'stmt> From<Insert<'stmt>> for Statement<'stmt> {
    fn from(src: Insert<'stmt>) -> Statement<'stmt> {
        Statement::Insert(src)
    }
}

impl<'stmt> Node<'stmt> for Insert<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_insert(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_insert(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_insert_mut(self);
    }
}
