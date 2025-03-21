use super::{Comma, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::Expr {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        use stmt::Expr::*;

        match self {
            Column(expr) => {
                let column = f.serializer.column_name(expr.column);
                fmt!(f, column);
            }
            Record(expr) => {
                let exprs = Comma(&expr.fields);
                fmt!(f, "(" exprs ")");
            }
            Value(expr) => expr.to_sql(f),
            _ => todo!("expr={:?}", self),
        }
    }
}

impl ToSql for &stmt::ExprOrderBy {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        if let Some(order) = &self.order {
            fmt!(f, self.expr " " order);
        } else {
            fmt!(f, self.expr);
        }
    }
}

impl ToSql for &stmt::Direction {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        match self {
            stmt::Direction::Asc => fmt!(f, "ASC"),
            stmt::Direction::Desc => fmt!(f, "DESC"),
        }
    }
}
