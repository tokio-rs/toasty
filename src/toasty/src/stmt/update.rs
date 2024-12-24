use super::*;

use std::{fmt, marker::PhantomData};

pub struct Update<M> {
    pub(crate) untyped: stmt::Update,
    _p: PhantomData<M>,
}

impl<M: Model> Update<M> {
    pub fn new(selection: Select<M>) -> Update<M> {
        let mut stmt = Update::default();
        stmt.set_selection(selection);
        stmt
    }

    pub const fn from_untyped(untyped: stmt::Update) -> Update<M> {
        Update {
            untyped,
            _p: PhantomData,
        }
    }

    pub fn set(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.set(field, expr);
    }

    pub fn insert(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.insert(field, expr);
    }

    pub fn remove(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.remove(field, expr);
    }

    pub fn set_selection(&mut self, selection: Select<M>) {
        let query = selection.untyped;

        match *query.body {
            stmt::ExprSet::Select(select) => {
                debug_assert_eq!(
                    select.source.as_model_id(),
                    self.untyped.target.as_model_id()
                );

                self.untyped.filter = Some(select.filter);
            }
            stmt::ExprSet::Values(values) => {
                self.untyped.filter = Some(stmt::Expr::in_list(stmt::Expr::key(M::ID), values.rows))
            }
            body => todo!("selection={body:#?}"),
        }
    }

    /// Don't return anything
    pub fn set_returning_none(&mut self) {
        self.untyped.returning = None;
    }
}

impl<M> Clone for Update<M> {
    fn clone(&self) -> Self {
        Update {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M: Model> Default for Update<M> {
    fn default() -> Self {
        Update {
            untyped: stmt::Update {
                target: stmt::UpdateTarget::Model(M::ID),
                assignments: stmt::Assignments::default(),
                filter: Some(stmt::Expr::from(false)),
                condition: None,
                returning: Some(stmt::Returning::Changed),
            },
            _p: PhantomData,
        }
    }
}

impl<M> AsRef<stmt::Update> for Update<M> {
    fn as_ref(&self) -> &stmt::Update {
        &self.untyped
    }
}

impl<M> AsMut<stmt::Update> for Update<M> {
    fn as_mut(&mut self) -> &mut stmt::Update {
        &mut self.untyped
    }
}

impl<M> fmt::Debug for Update<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
