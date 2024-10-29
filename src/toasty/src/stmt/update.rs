use super::*;

use std::{fmt, marker::PhantomData};

pub struct Update<'a, M> {
    pub(crate) untyped: stmt::Update<'a>,
    _p: PhantomData<M>,
}

impl<'a, M: Model> Update<'a, M> {
    /*
    pub fn new<S>(selection: S) -> Update<'a, M>
    where
        S: IntoSelect<'a, Model = M>,
    {
        let mut stmt = Update::default();
        stmt.untyped.selection = selection.into_select().untyped;
        stmt
    }
    */

    pub const fn from_untyped(untyped: stmt::Update<'a>) -> Update<'a, M> {
        Update {
            untyped,
            _p: PhantomData,
        }
    }

    /// Set the value of a specific field
    pub fn set(&mut self, field: usize, value: stmt::Value<'a>) {
        self.set_expr(field, value);
    }

    pub fn set_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'a>>) {
        self.untyped.assignments.set(field, expr);
    }

    pub fn push_expr(&mut self, field: usize, expr: impl Into<stmt::Expr<'a>>) {
        /*
        self.untyped.fields.insert(field);
        self.untyped.expr[field].push(expr);
        */
        todo!()
    }

    /*
    pub fn set_selection<S>(&mut self, selection: S)
    where
        S: IntoSelect<'a, Model = M>,
    {
        self.untyped.selection = selection.into_select().untyped;
    }
    */

    pub fn fields(&self) -> &stmt::PathFieldSet {
        &self.untyped.assignments.fields
    }
}

impl<M> Clone for Update<'_, M> {
    fn clone(&self) -> Self {
        Update {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<'a, M: Model> Default for Update<'a, M> {
    fn default() -> Self {
        Update {
            untyped: stmt::Update {
                target: stmt::UpdateTarget::Model(M::ID),
                assignments: stmt::Assignments::default(),
                filter: Some(stmt::Expr::from(false)),
                condition: None,
                returning: true,
            },
            _p: PhantomData,
        }
    }
}

impl<'a, M> AsRef<stmt::Update<'a>> for Update<'a, M> {
    fn as_ref(&self) -> &stmt::Update<'a> {
        &self.untyped
    }
}

impl<'a, M> AsMut<stmt::Update<'a>> for Update<'a, M> {
    fn as_mut(&mut self) -> &mut stmt::Update<'a> {
        &mut self.untyped
    }
}

impl<'a, M> fmt::Debug for Update<'a, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
