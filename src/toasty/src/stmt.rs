mod delete;
pub use delete::Delete;

mod expr;
pub use expr::Expr;

mod id;
pub use id::Id;

mod insert;
pub use insert::Insert;

mod into_expr;
pub use into_expr::IntoExpr;

mod into_insert;
pub use into_insert::IntoInsert;

mod into_select;
pub use into_select::AsSelect;
pub use into_select::IntoSelect;

mod key;
pub use key::Key;

mod link;
pub use link::Link;

mod path;
pub use path::Path;

mod select;
pub use select::Select;

mod to_statement;
pub use to_statement::ToStatement;

mod unlink;
pub use unlink::Unlink;

mod update;
pub use update::Update;

use crate::Model;

use toasty_core::stmt::{self, Value};

use std::{fmt, marker::PhantomData};

pub struct Statement<'a, M> {
    pub(crate) untyped: stmt::Statement<'a>,
    _p: PhantomData<M>,
}

impl<'a, M: Model> Statement<'a, M> {
    pub fn from_untyped(query: impl IntoSelect<'a, Model = M>) -> Statement<'a, M> {
        Statement {
            untyped: query.into_select().untyped.into(),
            _p: PhantomData,
        }
    }

    pub fn update<Q>(
        expr: stmt::ExprRecord<'a>,
        fields: stmt::PathFieldSet,
        selection: Q,
    ) -> Statement<'a, M>
    where
        Q: IntoSelect<'a, Model = M>,
    {
        /*
        let untyped = stmt::Update {
            fields,
            selection: selection.into_select().untyped,
            expr,
            condition: None,
            returning: true,
        }
        .into();

        Statement {
            untyped,
            _p: PhantomData,
        }
        */
        todo!()
    }
}

impl<'a, M> From<Select<'a, M>> for Statement<'a, M> {
    fn from(value: Select<'a, M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<'a, M> From<Insert<'a, M>> for Statement<'a, M> {
    fn from(value: Insert<'a, M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<'a, M> From<Update<'a, M>> for Statement<'a, M> {
    fn from(value: Update<'a, M>) -> Self {
        Statement {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<'a, M> fmt::Debug for Statement<'a, M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}

// TODO: move
pub fn in_set<'a, L, R, T>(lhs: L, rhs: R) -> Expr<'a, bool>
where
    L: IntoExpr<'a, T>,
    R: IntoExpr<'a, [T]>,
{
    Expr {
        untyped: stmt::Expr::in_list(lhs.into_expr().untyped, rhs.into_expr().untyped),
        _p: PhantomData,
    }
}
