use super::{
    Delete, ExprSet, Limit, Node, OrderBy, Path, Returning, Select, Source, Statement, Update,
    UpdateTarget, Values, Visit, VisitMut, With,
};
use crate::stmt::{self, Filter};

/// A query statement that reads data from the database.
///
/// `Query` wraps a set expression body (typically a [`Select`]) with optional
/// ordering, limits, CTEs, and row-level locks. It is the read-side counterpart
/// to [`Insert`], [`Update`], and [`Delete`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Query, Values, ExprSet};
///
/// // A unit query that returns one empty row
/// let q = Query::unit();
/// assert!(matches!(q.body, ExprSet::Values(_)));
/// assert!(!q.single);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Optional common table expressions (CTEs) for this query.
    pub with: Option<With>,

    /// The body of the query. Either `SELECT`, `UNION`, `VALUES`, or possibly
    /// other types of queries depending on database support.
    pub body: ExprSet,

    /// When `true`, the query returns a single record instead of a list.
    ///
    /// This is semantically different from `LIMIT 1`: it indicates there can
    /// only ever be one matching result. The return type becomes `Record`
    /// instead of `List`.
    pub single: bool,

    /// Optional `ORDER BY` clause.
    pub order_by: Option<OrderBy>,

    /// Optional `LIMIT` and `OFFSET` clause.
    pub limit: Option<Limit>,

    /// Row-level locks (`FOR UPDATE`, `FOR SHARE`).
    pub locks: Vec<Lock>,
}

/// A row-level lock to acquire when executing a query.
///
/// Corresponds to SQL's `FOR UPDATE` and `FOR SHARE` clauses. Only meaningful
/// for SQL databases that support row-level locking.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::Lock;
///
/// let lock = Lock::Update;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Lock {
    /// `FOR UPDATE` -- acquire an exclusive lock on matched rows.
    Update,
    /// `FOR SHARE` -- acquire a shared lock on matched rows.
    Share,
}

/// Builder for constructing [`Query`] instances with optional clauses.
///
/// Created via [`Query::builder`]. Allows chaining calls to add CTEs, filters,
/// returning clauses, and locks before calling [`build`](QueryBuilder::build).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Query, Select, Source, Filter};
///
/// let select = Select::new(Source::from(ModelId(0)), Filter::ALL);
/// let query = Query::builder(select).build();
/// ```
#[derive(Debug)]
pub struct QueryBuilder {
    query: Query,
}

impl Query {
    /// Creates a new query with the given body and default options (no ordering,
    /// no limit, not single, no locks).
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

    /// Creates a new query that returns exactly one record (`single = true`).
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

    /// Creates a new `SELECT` query from a source and filter.
    pub fn new_select(source: impl Into<Source>, filter: impl Into<Filter>) -> Self {
        Self::builder(Select::new(source, filter)).build()
    }

    /// Returns a [`QueryBuilder`] initialized with the given body.
    pub fn builder(body: impl Into<ExprSet>) -> QueryBuilder {
        QueryBuilder {
            query: Query::new(body),
        }
    }

    /// Creates a unit query that produces one empty row (empty `VALUES`).
    pub fn unit() -> Self {
        Query::new(Values::default())
    }

    /// Creates a query whose body is a `VALUES` expression.
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

    /// Converts this query into an [`Update`] statement targeting the same
    /// source. The query must have a `SELECT` body with a model source.
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

    /// Converts this query into a [`Delete`] statement. The query body must
    /// be a `SELECT`.
    pub fn delete(self) -> Delete {
        match self.body {
            ExprSet::Select(select) => Delete {
                from: select.source,
                filter: select.filter,
                returning: None,
                condition: Default::default(),
            },
            _ => todo!("{self:#?}"),
        }
    }

    /// Adds a filter to this query's `SELECT` body.
    ///
    /// # Panics
    ///
    /// Panics if the query body is not a `SELECT`.
    pub fn add_filter(&mut self, filter: impl Into<Filter>) {
        self.body.as_select_mut_unwrap().add_filter(filter);
    }

    /// Adds an association include path to this query's `SELECT` body.
    pub fn include(&mut self, path: impl Into<Path>) {
        match &mut self.body {
            ExprSet::Select(body) => body.include(path),
            _ => todo!(),
        }
    }
}

impl Statement {
    /// Returns `true` if this statement is a [`Query`].
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

    /// Consumes `self` and attempts to return the inner [`Query`].
    ///
    /// Returns `None` if `self` is not a [`Statement::Query`].
    pub fn into_query(self) -> Option<Query> {
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
    pub fn into_query_unwrap(self) -> Query {
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
    /// Sets the `WITH` (CTE) clause for the query being built.
    pub fn with(mut self, with: impl Into<With>) -> Self {
        self.query.with = Some(with.into());
        self
    }

    /// Sets the row-level locks for the query being built.
    pub fn locks(mut self, locks: impl Into<Vec<Lock>>) -> Self {
        self.query.locks = locks.into();
        self
    }

    /// Sets the filter on the query's `SELECT` body.
    ///
    /// # Panics
    ///
    /// Panics if the query body is not a `SELECT`.
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

    /// Sets the returning clause on the query's `SELECT` body.
    pub fn returning(mut self, returning: Returning) -> Self {
        match &mut self.query.body {
            ExprSet::Select(select) => {
                select.returning = returning;
            }
            _ => todo!(),
        }

        self
    }

    /// Consumes this builder and returns the constructed [`Query`].
    pub fn build(self) -> Query {
        self.query
    }
}

impl From<QueryBuilder> for Query {
    fn from(value: QueryBuilder) -> Self {
        value.build()
    }
}
