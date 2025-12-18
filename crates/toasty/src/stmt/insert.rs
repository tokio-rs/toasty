use super::{Expr, IntoSelect};
use crate::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

pub struct Insert<M: ?Sized> {
    pub(crate) untyped: stmt::Insert,
    _p: PhantomData<M>,
}

impl<M: Model> Insert<M> {
    /// Create an insertion statement that inserts an empty record
    /// (fields without #[auto] as `Expr::Value(Value::Null)`, #[auto] fields as `Expr::Default`).
    ///
    /// This insertion statement is not guaranteed to be valid.
    ///
    /// TODO: rename `new`?
    pub fn blank_single() -> Self {
        Self {
            untyped: stmt::Insert {
                target: stmt::InsertTarget::Model(M::id()),
                source: stmt::Query::new_single(vec![stmt::ExprRecord::from_vec(
                    M::schema()
                        .fields
                        .iter()
                        .map(|field| match field.auto() {
                            Some(_) => stmt::Expr::Default,
                            None => stmt::Expr::Value(stmt::Value::Null),
                        })
                        .collect(),
                )
                .into()]),
                returning: Some(stmt::Returning::Model { include: vec![] }),
            },
            _p: PhantomData,
        }
    }

    pub const fn from_untyped(untyped: stmt::Insert) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Set the scope of the insert.
    pub fn set_scope<S>(&mut self, scope: S)
    where
        S: IntoSelect<Model = M>,
    {
        self.untyped.target = stmt::InsertTarget::Scope(Box::new(scope.into_select().untyped));
    }

    pub fn set(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        *self.expr_mut(field) = expr.into();
    }

    /// Extend the expression for `field` with the given expression
    pub fn insert(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        // self.expr_mut(field).push(expr);
        let target = self.expr_mut(field);

        match target {
            stmt::Expr::Value(stmt::Value::Null) => {
                *target = stmt::Expr::list_from_vec(vec![expr.into()]);
            }
            stmt::Expr::List(expr_list) => {
                expr_list.items.push(expr.into());
            }
            _ => todo!("existing={target:#?}; expr={:#?}", expr.into()),
        }
    }

    pub(crate) fn merge(&mut self, stmt: Self) {
        self.untyped.merge(stmt.untyped);
    }

    fn expr_mut(&mut self, field: usize) -> &mut stmt::Expr {
        &mut self.current_mut()[field]
    }

    /// Returns the current record being updated
    fn current_mut(&mut self) -> &mut stmt::ExprRecord {
        let values = self.untyped.source.body.as_values_mut();
        values.rows.last_mut().unwrap().as_record_mut()
    }

    pub fn into_list_expr(self) -> Expr<[M]> {
        Expr::from_untyped(stmt::Expr::Stmt(self.untyped.into()))
    }
}

impl<M> Clone for Insert<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Insert<M>> for stmt::Expr {
    fn from(value: Insert<M>) -> Self {
        Self::stmt(value.untyped)
    }
}

impl<M> From<Insert<[M]>> for stmt::Expr {
    fn from(value: Insert<[M]>) -> Self {
        Self::stmt(value.untyped)
    }
}

impl<M> fmt::Debug for Insert<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
