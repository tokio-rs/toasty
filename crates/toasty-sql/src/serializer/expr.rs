use super::{Comma, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::Expr {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        use stmt::Expr::*;

        match self {
            BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                fmt!(f, expr.lhs " " expr.op " " expr.rhs);
            }
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

impl ToSql for &stmt::BinaryOp {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        f.dst.push_str(match self {
            stmt::BinaryOp::Eq => "=",
            stmt::BinaryOp::Gt => ">",
            stmt::BinaryOp::Ge => ">=",
            stmt::BinaryOp::Lt => "<",
            stmt::BinaryOp::Le => "<=",
            stmt::BinaryOp::Ne => "<>",
            _ => todo!(),
        })
    }
}

impl ToSql for &stmt::ExprOrderBy {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        if let Some(order) = &self.order {
            fmt!(f, self.expr " " order);
        } else {
            fmt!(f, self.expr);
        }
    }
}

impl ToSql for &stmt::Direction {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Direction::Asc => fmt!(f, "ASC"),
            stmt::Direction::Desc => fmt!(f, "DESC"),
        }
    }
}
