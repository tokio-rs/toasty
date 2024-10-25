use super::*;

use std::{fmt, marker::PhantomData};

pub struct Insert<'a, M: ?Sized> {
    pub(crate) untyped: stmt::Insert<'a>,
    _p: PhantomData<M>,
}

impl<'a, M: Model> Insert<'a, M> {
    /// Create an insertion statement that inserts an empty record (all fields are null).
    ///
    /// This insertion statement is not guaranteed to be valid.
    pub fn blank() -> Insert<'a, M> {
        Insert {
            untyped: stmt::Insert {
                target: stmt::InsertTarget::Model(M::ID),
                values: stmt::ExprRecord::from_vec(vec![stmt::Expr::record(
                    std::iter::repeat(stmt::Expr::null()).take(M::FIELD_COUNT),
                )])
                .into(),
                returning: Some(stmt::Returning::Star),
            },
            _p: PhantomData,
        }
    }

    pub const fn from_untyped(untyped: stmt::Insert<'a>) -> Insert<'a, M> {
        Insert {
            untyped,
            _p: PhantomData,
        }
    }

    /// Set the scope of the insert.
    pub fn set_scope<S>(&mut self, scope: S)
    where
        S: IntoSelect<'a, Model = M>,
    {
        self.untyped.target = stmt::InsertTarget::Scope(scope.into_select().untyped);
    }

    /// Set a record value for the last record in the statement
    pub fn set_value(&mut self, field: usize, value: impl Into<stmt::Value<'a>>) {
        self.current_mut()[field] = stmt::Expr::Value(value.into());
    }

    pub fn set_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'a>>) {
        self.current_mut()[field] = expr.into();
    }

    /// Extend the expression for `field` with the given expression
    pub fn push_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'a>>) {
        self.current_mut()[field].push(expr);
    }

    pub(crate) fn merge(&mut self, stmt: Insert<'a, M>) {
        self.untyped.merge(stmt.untyped);
    }

    /// Returns the current record being updated
    fn current_mut(&mut self) -> &mut stmt::ExprRecord<'a> {
        let stmt::Expr::Record(expr_record) = &mut self.untyped.values else {
            todo!()
        };
        expr_record.last_mut().unwrap().as_record_mut()
    }

    pub fn into_list_expr(self) -> Expr<'a, [M]> {
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

impl<'a, M> From<Insert<'a, M>> for stmt::Expr<'a> {
    fn from(value: Insert<'a, M>) -> Self {
        stmt::ExprStmt::new(value.untyped).into()
    }
}

impl<'a, M> From<Insert<'a, [M]>> for stmt::Expr<'a> {
    fn from(value: Insert<'a, [M]>) -> Self {
        stmt::ExprStmt::new(value.untyped).into()
    }
}

impl<'a, M> fmt::Debug for Insert<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
