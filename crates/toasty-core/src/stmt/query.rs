use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Any CTEs
    pub with: Option<With>,

    /// The body of the query. Either `SELECT`, `UNION`, `VALUES`, or possibly
    /// other types of queries depending on database support.
    pub body: Box<ExprSet>,
}

impl Query {
    pub fn unit() -> Query {
        Query {
            with: None,
            body: Box::new(ExprSet::Values(Values::default())),
        }
    }

    pub fn filter(source: impl Into<Source>, filter: impl Into<Expr>) -> Query {
        Query {
            with: None,
            body: Box::new(ExprSet::Select(Select::new(source, filter))),
        }
    }

    pub fn values(values: impl Into<Values>) -> Query {
        Query {
            with: None,
            body: Box::new(ExprSet::Values(values.into())),
        }
    }

    pub fn update(self) -> Update {
        let ExprSet::Select(select) = &*self.body else {
            todo!("stmt={self:#?}");
        };

        assert!(select.source.is_model());

        stmt::Update {
            target: UpdateTarget::Query(self),
            assignments: stmt::Assignments::default(),
            filter: None,
            condition: None,
            returning: None,
        }
    }

    pub fn delete(self) -> Delete {
        match *self.body {
            ExprSet::Select(select) => Delete {
                from: select.source,
                filter: select.filter,
                returning: None,
            },
            _ => todo!("{self:#?}"),
        }
    }

    pub fn and(&mut self, expr: impl Into<Expr>) {
        self.body.as_select_mut().and(expr);
    }

    pub fn union(&mut self, query: impl Into<Query>) {
        let rhs = query.into();

        match (&mut *self.body, *rhs.body) {
            (ExprSet::SetOp(_), ExprSet::SetOp(_)) => todo!(),
            (ExprSet::SetOp(lhs), rhs) if lhs.is_union() => {
                lhs.operands.push(rhs);
            }
            (_, ExprSet::SetOp(_)) => todo!(),
            (me, rhs) => {
                let lhs = std::mem::take(me);
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

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input) {
        self.body.substitute_ref(input);
    }
}

impl From<Query> for Statement {
    fn from(value: Query) -> Self {
        Statement::Query(value)
    }
}

impl Node for Query {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_stmt_query(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_stmt_query_mut(self);
    }
}
