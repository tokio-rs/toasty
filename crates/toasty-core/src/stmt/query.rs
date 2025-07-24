use super::*;

#[derive(Debug, Clone)]
pub struct Query {
    /// Any CTEs
    pub with: Option<With>,

    /// The body of the query. Either `SELECT`, `UNION`, `VALUES`, or possibly
    /// other types of queries depending on database support.
    pub body: ExprSet,

    /// ORDER BY
    pub order_by: Option<OrderBy>,

    /// LIMIT and OFFSET (count or keyset)
    pub limit: Option<Limit>,

    /// FOR { UPDATE | SHARE }
    pub locks: Vec<Lock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Lock {
    Update,
    Share,
}

#[derive(Debug)]
pub struct QueryBuilder {
    query: Query,
}

impl Query {
    pub fn new(body: impl Into<ExprSet>) -> Self {
        Self {
            with: None,
            body: body.into(),
            order_by: None,
            limit: None,
            locks: vec![],
        }
    }

    pub fn builder(body: impl Into<ExprSet>) -> QueryBuilder {
        QueryBuilder {
            query: Query::new(body),
        }
    }

    pub fn unit() -> Self {
        Query::new(Values::default())
    }

    pub fn filter(source: impl Into<Source>, filter: impl Into<Expr>) -> Self {
        Self::builder(Select::new(source, filter)).build()
    }

    pub fn values(values: impl Into<Values>) -> Self {
        Self {
            with: None,
            body: ExprSet::Values(values.into()),
            order_by: None,
            limit: None,
            locks: vec![],
        }
    }

    pub fn update(self) -> Update {
        let ExprSet::Select(select) = &self.body else {
            todo!("stmt={self:#?}");
        };

        assert!(select.source.is_model());

        stmt::Update {
            target: UpdateTarget::Query(Box::new(self)),
            assignments: stmt::Assignments::default(),
            filter: None,
            condition: None,
            returning: None,
        }
    }

    pub fn delete(self) -> Delete {
        match self.body {
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

    pub fn union(&mut self, query: impl Into<Self>) {
        let rhs = query.into();

        match (&mut self.body, rhs.body) {
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
        match &mut self.body {
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
        Self::Query(value)
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

impl QueryBuilder {
    pub fn with(mut self, with: impl Into<With>) -> Self {
        self.query.with = Some(with.into());
        self
    }

    pub fn locks(mut self, locks: impl Into<Vec<Lock>>) -> Self {
        self.query.locks = locks.into();
        self
    }

    pub fn filter(mut self, filter: impl Into<Expr>) -> Self {
        let filter = filter.into();

        match &mut self.query.body {
            ExprSet::Select(select) => {
                select.filter = filter;
            }
            _ => todo!(),
        }

        self
    }

    pub fn returning(mut self, returning: impl Into<Returning>) -> Self {
        let returning = returning.into();

        match &mut self.query.body {
            ExprSet::Select(select) => {
                select.returning = returning;
            }
            _ => todo!(),
        }

        self
    }

    pub fn build(self) -> Query {
        self.query
    }
}
