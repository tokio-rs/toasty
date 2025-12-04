use super::{Expr, Path};
use crate::stmt::{self, ExprSet, Node, Query, Statement, Value};

/// TODO: rename since this is also used in `Select`?
#[derive(Debug, Clone, PartialEq)]
pub enum Returning {
    /// Return the full model with specified includes
    Model {
        include: Vec<Path>,
    },

    Changed,

    /// Return an expression.
    Expr(Expr),

    /// Return a value instead of a projection of the statement source.
    Value(Expr),
}

impl Returning {
    pub fn from_expr_iter<T>(items: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<Expr>,
    {
        Returning::Expr(Expr::record(items))
    }

    pub fn is_model(&self) -> bool {
        matches!(self, Self::Model { .. })
    }

    pub fn as_model_includes(&self) -> &[Path] {
        match self {
            Self::Model { include } => include,
            _ => &[],
        }
    }

    pub fn as_model_includes_mut(&mut self) -> &mut Vec<Path> {
        match self {
            Self::Model { include } => include,
            _ => panic!("not a Model variant"),
        }
    }

    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed)
    }

    pub fn is_expr(&self) -> bool {
        matches!(self, Self::Expr(_))
    }

    pub fn as_expr(&self) -> Option<&Expr> {
        match self {
            Self::Expr(expr) => Some(expr),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_expr_unwrap(&self) -> &Expr {
        match self {
            Self::Expr(expr) => expr,
            _ => panic!("expected stmt::Returning::Expr; actual={self:#?}"),
        }
    }

    pub fn as_expr_mut(&mut self) -> Option<&mut Expr> {
        match self {
            Self::Expr(expr) => Some(expr),
            _ => None,
        }
    }

    #[track_caller]
    pub fn as_expr_mut_unwrap(&mut self) -> &mut Expr {
        match self {
            Self::Expr(expr) => expr,
            _ => panic!("expected stmt::Returningm::Expr; actual={self:#?}"),
        }
    }

    pub fn set_expr(&mut self, expr: impl Into<Expr>) {
        *self = Returning::Expr(expr.into());
    }

    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value(..))
    }

    /// Replaces this value with `Returning::Expr(null)` and returns the original value.
    pub fn take(&mut self) -> Returning {
        std::mem::replace(self, Returning::Expr(stmt::Expr::null()))
    }
}

impl Statement {
    /// Returns a reference to this statement's `RETURNING` clause, if present.
    ///
    /// Returns `None` if the statement does not have a `RETURNING` clause or is
    /// a statement type that does not support `RETURNING`.
    pub fn returning(&self) -> Option<&Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.as_ref(),
            Statement::Insert(insert) => insert.returning.as_ref(),
            Statement::Query(query) => query.returning(),
            Statement::Update(update) => update.returning.as_ref(),
        }
    }

    /// Take the `Returning` clause
    pub fn take_returning(&mut self) -> Option<Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.take(),
            Statement::Insert(insert) => insert.returning.take(),
            Statement::Query(query) => match &mut query.body {
                ExprSet::Select(select) => Some(select.returning.take()),
                ExprSet::Values(..) => None,
                _ => todo!("stmt={self:#?}"),
            },
            Statement::Update(update) => update.returning.take(),
        }
    }

    /// Set the `Returning` clause
    pub fn set_returning(&mut self, returning: Returning) {
        match self {
            Statement::Delete(delete) => delete.returning = Some(returning),
            Statement::Insert(insert) => insert.returning = Some(returning),
            Statement::Query(query) => *query.returning_mut_unwrap() = returning,
            Statement::Update(update) => update.returning = Some(returning),
        }
    }

    /// Returns a reference to this statement's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not have a `RETURNING` clause.
    #[track_caller]
    pub fn returning_unwrap(&self) -> &Returning {
        self.returning().unwrap_or_else(|| {
            panic!("expected statement to have RETURNING clause; actual={self:#?}")
        })
    }

    /// Returns a mutable reference to this statement's `RETURNING` clause, if present.
    ///
    /// Returns `None` if the statement does not have a `RETURNING` clause or is
    /// a statement type that does not support `RETURNING`.
    pub fn returning_mut(&mut self) -> Option<&mut Returning> {
        match self {
            Statement::Delete(delete) => delete.returning.as_mut(),
            Statement::Insert(insert) => insert.returning.as_mut(),
            Statement::Query(query) => query.returning_mut(),
            Statement::Update(update) => update.returning.as_mut(),
        }
    }

    /// Returns a mutable reference to this statement's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the statement does not have a `RETURNING` clause. This can occur when:
    /// - A `DELETE`, `INSERT`, or `UPDATE` statement was created without specifying a
    ///   `RETURNING` clause (the internal `Option<Returning>` is `None`)
    /// - A `Query` statement contains a non-`SELECT` body (e.g., `VALUES`, `UNION`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut stmt = Statement::Insert(insert_with_returning);
    /// let returning = stmt.returning_mut_unwrap();
    /// // Modify the returning clause...
    /// ```
    ///
    /// # Notes
    ///
    /// This method uses `#[track_caller]` to report the panic location at the call site
    /// rather than inside this method, making debugging easier.
    #[track_caller]
    pub fn returning_mut_unwrap(&mut self) -> &mut Returning {
        match self {
            Statement::Delete(delete) => delete.returning.as_mut().unwrap(),
            Statement::Insert(insert) => insert.returning.as_mut().unwrap(),
            Statement::Query(query) => query.returning_mut_unwrap(),
            Statement::Update(update) => update.returning.as_mut().unwrap(),
        }
    }
}

impl Query {
    /// Returns a reference to this query's `RETURNING` clause, if present.
    ///
    /// Returns `Some` only for `SELECT` queries. Other query types (`VALUES`,
    /// `UNION`, etc.) do not have a `RETURNING` clause.
    pub fn returning(&self) -> Option<&Returning> {
        match &self.body {
            stmt::ExprSet::Select(select) => Some(&select.returning),
            _ => None,
        }
    }

    /// Returns a reference to this query's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the query does not have a `RETURNING` clause (i.e., the body
    /// is not a `SELECT`).
    #[track_caller]
    pub fn returning_unwrap(&self) -> &Returning {
        self.returning()
            .unwrap_or_else(|| panic!("expected query to have RETURNING clause; actual={self:#?}"))
    }

    /// Returns a mutable reference to this query's `RETURNING` clause, if present.
    ///
    /// Returns `Some` only for `SELECT` queries. Other query types (`VALUES`,
    /// `UNION`, etc.) do not have a `RETURNING` clause.
    pub fn returning_mut(&mut self) -> Option<&mut Returning> {
        match &mut self.body {
            stmt::ExprSet::Select(select) => Some(&mut select.returning),
            _ => None,
        }
    }

    /// Returns a mutable reference to this query's `RETURNING` clause.
    ///
    /// # Panics
    ///
    /// Panics if the query does not have a `RETURNING` clause (i.e., the body
    /// is not a `SELECT`).
    #[track_caller]
    pub fn returning_mut_unwrap(&mut self) -> &mut Returning {
        match &mut self.body {
            stmt::ExprSet::Select(select) => &mut select.returning,
            body => panic!("expected query to have RETURNING clause; actual={body:#?}"),
        }
    }
}

impl<T> From<T> for Returning
where
    Value: From<T>,
{
    fn from(value: T) -> Self {
        Returning::Expr(Value::from(value).into())
    }
}

impl From<Expr> for Returning {
    fn from(value: Expr) -> Self {
        Self::Expr(value)
    }
}

impl From<Vec<Expr>> for Returning {
    fn from(value: Vec<Expr>) -> Self {
        stmt::Returning::Expr(stmt::Expr::record_from_vec(value))
    }
}

impl Node for Returning {
    fn visit<V: stmt::Visit>(&self, mut visit: V)
    where
        Self: Sized,
    {
        visit.visit_returning(self);
    }

    fn visit_mut<V: stmt::VisitMut>(&mut self, mut visit: V) {
        visit.visit_returning_mut(self);
    }
}
