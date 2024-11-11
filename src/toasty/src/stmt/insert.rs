use super::*;

use std::{fmt, marker::PhantomData};

pub struct Insert<'stmt, M: ?Sized> {
    pub(crate) untyped: stmt::Insert<'stmt>,
    _p: PhantomData<M>,
}

impl<'stmt, M: Model> Insert<'stmt, M> {
    /// Create an insertion statement that inserts an empty record (all fields are null).
    ///
    /// This insertion statement is not guaranteed to be valid.
    ///
    /// TODO: rename `new`?
    pub fn blank() -> Insert<'stmt, M> {
        Insert {
            untyped: stmt::Insert {
                target: stmt::InsertTarget::Model(M::ID),
                source: stmt::Query {
                    body: Box::new(stmt::ExprSet::Values(stmt::Values::new(vec![
                        stmt::ExprRecord::from_vec(vec![]).into(),
                    ]))),
                },
                returning: Some(stmt::Returning::Star),
            },
            _p: PhantomData,
        }
    }

    pub const fn from_untyped(untyped: stmt::Insert<'stmt>) -> Insert<'stmt, M> {
        Insert {
            untyped,
            _p: PhantomData,
        }
    }

    /// Set the scope of the insert.
    pub fn set_scope<S>(&mut self, scope: S)
    where
        S: IntoSelect<'stmt, Model = M>,
    {
        self.untyped.target = stmt::InsertTarget::Scope(scope.into_select().untyped);
    }

    /// Set a record value for the last record in the statement
    pub fn set_value(&mut self, field: usize, value: impl Into<stmt::Value<'stmt>>) {
        self.set_expr(field, stmt::Expr::Value(value.into()));
    }

    pub fn set_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'stmt>>) {
        *self.expr_mut(field) = expr.into();
    }

    /// Extend the expression for `field` with the given expression
    pub fn push_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'stmt>>) {
        self.current_mut()[field].push(expr);
    }

    pub(crate) fn merge(&mut self, stmt: Insert<'stmt, M>) {
        self.untyped.merge(stmt.untyped);
    }

    fn expr_mut(&mut self, field: usize) -> &mut stmt::Expr<'stmt> {
        let row = self.current_mut();

        while row.fields.len() <= field {
            row.fields.push(stmt::Expr::default());
        }

        &mut row[field]
    }

    /// Returns the current record being updated
    fn current_mut(&mut self) -> &mut stmt::ExprRecord<'stmt> {
        let values = self.untyped.source.body.as_values_mut();
        values.rows.last_mut().unwrap().as_record_mut()
    }

    pub fn into_list_expr(self) -> Expr<'stmt, [M]> {
        Expr::from_untyped(stmt::Expr::Stmt(self.untyped.into()))
    }
}

impl<M> Clone for Insert<'_, M> {
    fn clone(&self) -> Self {
        Insert {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<'stmt, M> From<Insert<'stmt, M>> for stmt::Expr<'stmt> {
    fn from(value: Insert<'stmt, M>) -> Self {
        stmt::ExprStmt::new(value.untyped).into()
    }
}

impl<'stmt, M> From<Insert<'stmt, [M]>> for stmt::Expr<'stmt> {
    fn from(value: Insert<'stmt, [M]>) -> Self {
        stmt::ExprStmt::new(value.untyped).into()
    }
}

impl<'stmt, M> fmt::Debug for Insert<'stmt, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
