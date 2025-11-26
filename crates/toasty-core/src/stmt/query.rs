use super::{
    Delete, ExprSet, Limit, Node, OrderBy, Path, Returning, Select, Source, Statement, Update,
    UpdateTarget, Values, Visit, VisitMut, With,
};
use crate::stmt::{self, ExprSetOp, Filter, SetOp};

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Any CTEs
    pub with: Option<With>,

    /// The body of the query. Either `SELECT`, `UNION`, `VALUES`, or possibly
    /// other types of queries depending on database support.
    pub body: ExprSet,

    /// When `true`, the Query returns a *single* record vs. a list. Note, that
    /// this is different from `LIMIT 1` as there should only ever be 1 possible
    /// result. Also, the return type becomes `Record` instead of `List`.
    pub single: bool,

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
            single: false,
            order_by: None,
            limit: None,
            locks: vec![],
        }
    }

    pub fn new_single(body: impl Into<ExprSet>) -> Self {
        Self {
            with: None,
            body: body.into(),
            single: true,
            order_by: None,
            limit: None,
            locks: vec![],
        }
    }

    pub fn new_select(source: impl Into<Source>, filter: impl Into<Filter>) -> Self {
        Self::builder(Select::new(source, filter)).build()
    }

    pub fn builder(body: impl Into<ExprSet>) -> QueryBuilder {
        QueryBuilder {
            query: Query::new(body),
        }
    }

    pub fn unit() -> Self {
        Query::new(Values::default())
    }

    pub fn values(values: impl Into<Values>) -> Self {
        Self {
            with: None,
            body: ExprSet::Values(values.into()),
            single: false,
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
            filter: Filter::default(),
            condition: stmt::Condition::default(),
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

    pub fn add_filter(&mut self, filter: impl Into<Filter>) {
        self.body.as_select_mut_unwrap().add_filter(filter);
    }

    pub fn add_union(&mut self, other: impl Into<Self>) {
        let rhs = other.into();

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
}

impl Statement {
    pub fn is_query(&self) -> bool {
        matches!(self, Statement::Query(_))
    }

    /// Attempts to return a reference to an inner [`Query`].
    ///
    /// * If `self` is a [`Statement::Query`], a reference to the inner [`Query`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_query(&self) -> Option<&Query> {
        match self {
            Self::Query(query) => Some(query),
            _ => None,
        }
    }

    /// Returns a mutable reference to the inner [`Query`], if this is a query statement.
    ///
    /// * If `self` is a [`Statement::Query`], a mutable reference to the inner [`Query`] is
    ///   returned wrapped in [`Some`].
    /// * Else, [`None`] is returned.
    pub fn as_query_mut(&mut self) -> Option<&mut Query> {
        match self {
            Self::Query(query) => Some(query),
            _ => None,
        }
    }

    /// Consumes `self` and returns the inner [`Query`].
    ///
    /// # Panics
    ///
    /// If `self` is not a [`Statement::Query`].
    #[track_caller]
    pub fn into_query(self) -> Query {
        match self {
            Self::Query(query) => query,
            v => panic!("expected `Query`, found {v:#?}"),
        }
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

    pub fn filter(mut self, filter: impl Into<Filter>) -> Self {
        let filter = filter.into();

        match &mut self.query.body {
            ExprSet::Select(select) => {
                select.filter = filter;
            }
            _ => todo!("query={self:#?}"),
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

impl From<QueryBuilder> for Query {
    fn from(value: QueryBuilder) -> Self {
        value.build()
    }
}
