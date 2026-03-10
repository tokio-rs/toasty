mod association;
pub use association::Association;

mod delete;
pub use delete::Delete;

mod expr;
pub use expr::Expr;

mod insert;
pub use insert::Insert;

mod into_expr;
pub use into_expr::IntoExpr;

mod into_insert;
pub use into_insert::IntoInsert;

mod into_select;
pub use into_select::IntoSelect;

mod into_statement;
pub use into_statement::IntoStatement;

mod paginate;
pub use paginate::Paginate;

mod path;
pub use path::Path;

pub use crate::model::{Auto, Field};

mod select;
pub use select::Select;

mod update;
pub use update::Update;

pub use toasty_core::stmt::{OrderBy, Projection, Value};

use crate::Model;

use toasty_core::stmt;

use std::{fmt, marker::PhantomData};

pub struct Statement<M> {
    pub(crate) untyped: stmt::Statement,
    _p: PhantomData<M>,
}

impl<M> Statement<M> {
    /// Wrap a raw untyped statement.
    pub fn from_untyped_stmt(untyped: stmt::Statement) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }
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
