use super::{List, Query};
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

pub struct Update<M> {
    pub(crate) untyped: stmt::Update,
    _p: PhantomData<M>,
}

// Methods available on all Update<M> regardless of M
impl<M> Update<M> {
    pub const fn from_untyped(untyped: stmt::Update) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    pub fn as_untyped_mut(&mut self) -> &mut stmt::Update {
        &mut self.untyped
    }

    pub fn set_assignments(&mut self, assignments: stmt::Assignments) {
        self.untyped.assignments = assignments;
    }

    pub fn set(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.set(field, expr);
    }

    pub fn insert(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.insert(field, expr);
    }

    pub fn remove(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.remove(field, expr);
    }

    /// Don't return anything
    pub fn set_returning_none(&mut self) {
        self.untyped.returning = None;
    }

    /// Consume this typed update and return the untyped core statement.
    pub fn into_untyped_stmt(self) -> stmt::Statement {
        self.untyped.into()
    }
}

/// Construct an `Update<List<M>>` for query-based updates that can affect
/// multiple rows.
impl<M: Model> Update<List<M>> {
    pub fn new(mut selection: Query<M>) -> Self {
        if let stmt::ExprSet::Values(values) = &mut selection.untyped.body {
            let rows = std::mem::take(&mut values.rows);
            let filter = stmt::Expr::in_list(stmt::Expr::ref_ancestor_model(0), rows);
            selection.untyped.body =
                stmt::ExprSet::Select(Box::new(stmt::Select::new(M::id(), filter)));
        }

        let mut stmt = selection.untyped.update();
        stmt.returning = Some(stmt::Returning::Changed);

        Self {
            untyped: stmt,
            _p: PhantomData,
        }
    }
}

/// Construct an `Update<M>` for single-instance updates that return exactly
/// one row.
impl<M: Model> Update<M> {
    pub fn new_single(mut selection: Query<M>) -> Self {
        if let stmt::ExprSet::Values(values) = &mut selection.untyped.body {
            let rows = std::mem::take(&mut values.rows);
            let filter = stmt::Expr::in_list(stmt::Expr::ref_ancestor_model(0), rows);
            selection.untyped.body =
                stmt::ExprSet::Select(Box::new(stmt::Select::new(M::id(), filter)));
        }

        let mut stmt = selection.untyped.update();
        stmt.returning = Some(stmt::Returning::Changed);

        Self {
            untyped: stmt,
            _p: PhantomData,
        }
    }
}

impl<M> Clone for Update<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M: Model> Default for Update<M> {
    fn default() -> Self {
        Self {
            untyped: stmt::Update {
                target: stmt::UpdateTarget::Model(M::id()),
                assignments: stmt::Assignments::default(),
                filter: stmt::Filter::new(stmt::Expr::from(false)),
                condition: stmt::Condition::default(),
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
