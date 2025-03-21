mod expr;
pub use expr::Expr;

mod into_expr;
pub use into_expr::IntoExpr;

use crate::{stmt, Model};

pub trait IntoQuery<'a> {
    type Model: Model;

    fn into_query(self) -> stmt::Query<'a, Self::Model>;
}

impl<'a, M: Model> IntoQuery<'a> for stmt::Query<'a, M> {
    type Model = M;

    fn into_query(self) -> stmt::Query<'a, Self::Model> {
        self
    }
}
