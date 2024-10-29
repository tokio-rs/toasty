use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Query<'stmt> {
    pub body: Box<ExprSet<'stmt>>,
}

impl<'stmt> Query<'stmt> {
    pub fn unit() -> Query<'stmt> {
        Query {
            body: Box::new(ExprSet::Values(Values::default())),
        }
    }

    pub fn filter(source: impl Into<Source>, filter: impl Into<Expr<'stmt>>) -> Query<'stmt> {
        Query {
            body: Box::new(ExprSet::Select(Select::new(source, filter))),
        }
    }

    pub fn update(self, schema: &Schema) -> Update<'stmt> {
        let ExprSet::Select(select) = *self.body else {
            todo!()
        };
        let width = schema.model(select.source.as_model_id()).fields.len();

        stmt::Update {
            target: UpdateTarget::Model(select.source.as_model_id()),
            assignments: stmt::Assignments::with_capacity(width),
            filter: Some(select.filter),
            condition: None,
            returning: None,
        }
    }

    pub fn delete(self) -> Delete<'stmt> {
        match *self.body {
            ExprSet::Select(select) => Delete {
                from: select.source,
                filter: select.filter,
                returning: None,
            },
            _ => todo!("{self:#?}"),
        }
    }

    pub fn and(&mut self, expr: impl Into<Expr<'stmt>>) {
        self.body.as_select_mut().and(expr);
    }

    pub fn union(&mut self, query: impl Into<Query<'stmt>>) {
        use std::mem;

        let rhs = query.into();

        match (&mut *self.body, *rhs.body) {
            (ExprSet::SetOp(_), ExprSet::SetOp(_)) => todo!(),
            (ExprSet::SetOp(lhs), rhs) if lhs.is_union() => {
                lhs.operands.push(rhs);
            }
            (_, ExprSet::SetOp(_)) => todo!(),
            (me, rhs) => {
                let lhs = mem::replace(me, ExprSet::default());
                *me = ExprSet::SetOp(ExprSetOp {
                    op: SetOp::Union,
                    operands: vec![lhs, rhs],
                });
            }
        }
    }

    pub fn include(&mut self, path: impl Into<Path>) {
        match &mut *self.body {
            ExprSet::Select(body) => body.include(path),
            _ => todo!(),
        }
    }

    pub fn substitute(&mut self, mut input: impl substitute::Input<'stmt>) {
        self.substitute_ref(&mut input);
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        self.body.substitute_ref(input);
    }
}

impl<'stmt> From<Query<'stmt>> for Statement<'stmt> {
    fn from(value: Query<'stmt>) -> Self {
        Statement::Query(value)
    }
}

impl<'stmt> Node<'stmt> for Query<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_stmt_query(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_stmt_query(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_stmt_query_mut(self);
    }
}
