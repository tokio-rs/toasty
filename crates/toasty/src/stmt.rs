mod association;
pub use association::Association;

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
pub use into_select::IntoSelect;

mod paginate;
pub use paginate::Paginate;

mod path;
pub use path::Path;

mod primitive;
pub use primitive::{Auto, Primitive};

#[cfg(feature = "jiff")]
mod primitive_jiff;

mod select;
pub use select::Select;

mod to_statement;
pub use to_statement::ToStatement;

mod update;
pub use update::Update;

pub use toasty_core::stmt::{OrderBy, Value};

use crate::Model;

use toasty_core::stmt;

use std::{fmt, marker::PhantomData};

pub struct Statement<M> {
    pub(crate) untyped: stmt::Statement,
    _p: PhantomData<M>,
}

impl<M: Model> Statement<M> {
    pub fn from_untyped(query: impl IntoSelect<Model = M>) -> Self {
        Self {
            untyped: query.into_select().untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Select<M>> for Statement<M> {
    fn from(value: Select<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Insert<M>> for Statement<M> {
    fn from(value: Insert<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Update<M>> for Statement<M> {
    fn from(value: Update<M>) -> Self {
        Self {
            untyped: value.untyped.into(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Statement<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
