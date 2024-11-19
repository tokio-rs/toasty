use super::*;
use toasty_core::schema::FieldId;

use std::{fmt, marker::PhantomData};

pub struct Update<M> {
    pub(crate) untyped: stmt::Update,
    _p: PhantomData<M>,
}

impl<M: Model> Update<M> {
    pub fn new<S>(selection: S) -> Update<M>
    where
        S: IntoSelect<Model = M>,
    {
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

    /// Set the value of a specific field
    pub fn set(&mut self, field: usize, value: stmt::Value) {
        self.set_expr(field, value);
    }

    pub fn set_expr(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.set(field, expr);
    }

    pub fn push_expr(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        /*
        self.untyped.fields.insert(field);
        self.untyped.expr[field].push(expr);
        */
        todo!()
    }

    pub fn set_selection<S>(&mut self, selection: S)
    where
        S: IntoSelect<Model = M>,
    {
        let select = selection.into_select().untyped;

        match *select.body {
            stmt::ExprSet::Select(select) => {
                assert_eq!(
                    select.source.as_model_id(),
                    self.untyped.target.as_model_id()
                );
                self.untyped.filter = Some(select.filter);
            }
            _ => todo!("selection={select:#?}"),
        }
    }

    pub fn fields(&self) -> &stmt::PathFieldSet {
        &self.untyped.assignments.fields
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
