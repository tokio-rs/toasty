use crate::stmt::{Expr, ExprSet, Node, Query, Statement, Visit, VisitMut};

/// A `WHERE` clause filter for statements.
///
/// Wraps an optional expression. When `expr` is `None`, the filter matches all
/// rows (equivalent to `WHERE true`). Filters can be combined with
/// [`add_filter`](Filter::add_filter), which AND-s the expressions together.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::Filter;
///
/// // An empty filter matches everything
/// let filter = Filter::default();
/// assert!(filter.expr.is_none());
///
/// // Filter::ALL is a const alias for the same thing
/// assert!(Filter::ALL.expr.is_none());
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filter {
    /// The filter expression, or `None` to match all rows.
    pub expr: Option<Expr>,
}

impl Filter {
    /// A filter that matches all rows (no expression set).
    pub const ALL: Filter = Filter { expr: None };

    /// Creates a filter from an expression.
    pub fn new(expr: impl Into<Expr>) -> Filter {
        Filter {
            expr: Some(expr.into()),
        }
    }

    /// Creates a filter by AND-ing two filters together.
    pub fn and(lhs: impl Into<Filter>, rhs: impl Into<Filter>) -> Filter {
        let mut ret = lhs.into();
        ret.add_filter(rhs);
        ret
    }

    /// Returns the filter expression.
    ///
    /// When no expression is set, returns `true`, which matches all rows.
    pub fn as_expr(&self) -> &Expr {
        self.expr.as_ref().unwrap_or(&Expr::TRUE)
    }

    /// Consumes the filter and returns the expression, defaulting to `true`.
    pub fn into_expr(self) -> Expr {
        self.expr.unwrap_or(Expr::TRUE)
    }

    /// Returns `true` if the filter expression is the literal `false`.
    pub fn is_false(&self) -> bool {
        self.expr
            .as_ref()
            .map(|expr| expr.is_false())
            .unwrap_or(false)
    }

    /// Replaces the filter expression with the given expression.
    pub fn set(&mut self, expr: impl Into<Expr>) {
        self.expr = Some(expr.into());
    }

    /// Adds a filter by AND-ing it with the current expression.
    ///
    /// If either filter is empty, the other is used directly.
    pub fn add_filter(&mut self, filter: impl Into<Filter>) {
        match (self.expr.take(), filter.into().expr) {
            (Some(expr), Some(other)) => {
                self.expr = Some(Expr::and(expr, other));
            }
            (Some(expr), None) => {
                self.expr = Some(expr);
            }
            (_, other) => {
                self.expr = other;
            }
        }
    }

    /// Takes the filter out, leaving an empty filter in its place.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toasty_core::stmt::Filter;
    /// let mut filter = Filter::default();
    /// let taken = filter.take();
    /// assert!(filter.expr.is_none());
    /// ```
    pub fn take(&mut self) -> Filter {
        Filter {
            expr: self.expr.take(),
        }
    }
}

impl Statement {
    /// Returns a reference to this statement's filter, if it has one.
    ///
    /// Returns `None` for `INSERT` statements.
    pub fn filter(&self) -> Option<&Filter> {
        match self {
            Statement::Delete(delete) => Some(&delete.filter),
            Statement::Insert(_) => None,
            Statement::Query(query) => query.filter(),
            Statement::Update(update) => Some(&update.filter),
        }
    }

    #[track_caller]
    pub fn filter_unwrap(&self) -> &Filter {
        match self.filter() {
            Some(filter) => filter,
            _ => panic!("expected statement to have a filter; statement={self:#?}"),
        }
    }

    pub fn filter_or_default(&self) -> &Filter {
        self.filter().unwrap_or(&Filter::ALL)
    }

    /// Returns a mutable reference to the statement's filter.
    ///
    /// Returns `None` for statements that do not support filtering, such as
    /// `INSERT`.
    pub fn filter_mut(&mut self) -> Option<&mut Filter> {
        match self {
            Statement::Delete(delete) => Some(&mut delete.filter),
            Statement::Insert(_) => None,
            Statement::Query(query) => query.filter_mut(),
            Statement::Update(update) => Some(&mut update.filter),
        }
    }

    /// Returns a mutable reference to the statement's filter.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not support filtering.
    #[track_caller]
    pub fn filter_mut_unwrap(&mut self) -> &mut Filter {
        match self {
            Statement::Delete(delete) => &mut delete.filter,
            Statement::Insert(_) => panic!("expected Statement with filter"),
            Statement::Query(query) => query.filter_mut_unwrap(),
            Statement::Update(update) => &mut update.filter,
        }
    }

    #[track_caller]
    pub fn filter_expr_unwrap(&self) -> &Expr {
        self.filter()
            .and_then(|f| f.expr.as_ref())
            .expect("expected Statement with expression filter")
    }

    pub fn filter_expr_mut(&mut self) -> Option<&mut Expr> {
        self.filter_mut().and_then(|filter| filter.expr.as_mut())
    }
}

impl Query {
    pub fn filter(&self) -> Option<&Filter> {
        match &self.body {
            ExprSet::Select(select) => Some(&select.filter),
            _ => None,
        }
    }

    #[track_caller]
    pub fn filter_unwrap(&self) -> &Filter {
        self.filter()
            .unwrap_or_else(|| panic!("expected Query with filter; actual={self:#?}"))
    }

    /// Returns a mutable reference to the query's filter.
    ///
    /// Returns `None` for queries that are not `SELECT` statements, such as
    /// `UNION` or `VALUES`.
    pub fn filter_mut(&mut self) -> Option<&mut Filter> {
        match &mut self.body {
            ExprSet::Select(select) => Some(&mut select.filter),
            _ => None,
        }
    }

    /// Returns a mutable reference to the query's filter.
    ///
    /// # Panics
    ///
    /// Panics if the query body is not a `SELECT` statement.
    #[track_caller]
    pub fn filter_mut_unwrap(&mut self) -> &mut Filter {
        match &mut self.body {
            ExprSet::Select(select) => &mut select.filter,
            _ => panic!("expected Query with filter"),
        }
    }
}

impl Node for Filter {
    fn visit<V: Visit>(&self, mut visit: V) {
        visit.visit_filter(self);
    }

    fn visit_mut<V: VisitMut>(&mut self, mut visit: V) {
        visit.visit_filter_mut(self);
    }
}

impl<T> From<T> for Filter
where
    Expr: From<T>,
{
    fn from(value: T) -> Self {
        Filter {
            expr: Some(value.into()),
        }
    }
}
