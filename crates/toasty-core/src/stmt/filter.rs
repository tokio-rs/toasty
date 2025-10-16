use crate::stmt::{Expr, ExprSet, Node, Query, Statement, Visit, VisitMut};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filter {
    pub expr: Option<Expr>,
}

impl Filter {
    pub fn new(expr: impl Into<Expr>) -> Filter {
        Filter {
            expr: Some(expr.into()),
        }
    }

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

    pub fn into_expr(self) -> Expr {
        self.expr.unwrap_or(Expr::TRUE)
    }

    pub fn is_false(&self) -> bool {
        self.expr
            .as_ref()
            .map(|expr| expr.is_false())
            .unwrap_or(false)
    }

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
    pub fn filter(&self) -> Option<&Filter> {
        match self {
            Statement::Delete(delete) => Some(&delete.filter),
            Statement::Insert(_) => None,
            Statement::Query(query) => query.filter(),
            Statement::Update(update) => Some(&update.filter),
        }
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
