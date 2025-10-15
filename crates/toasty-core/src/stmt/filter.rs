use crate::stmt::{Expr, ExprSet, Statement};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Filter {
    expr: Option<Expr>,
}

impl Filter {
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
}

impl Statement {
    pub fn filter(&self) -> Option<&Filter> {
        match self {
            Statement::Delete(delete) => Some(&delete.filter),
            Statement::Insert(_) => None,
            Statement::Query(query) => match &query.body {
                ExprSet::Select(select) => Some(&select.filter),
                _ => None,
            },
            Statement::Update(update) => Some(&update.filter),
        }
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
