use super::{Comma, Delimited, Params, ToSql};

use crate::stmt;

pub(super) struct ExprAsList<'a>(&'a stmt::Expr);

pub(super) struct ValueAsList<'a>(&'a stmt::Value);

impl ToSql for &stmt::Expr {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        use stmt::Expr::*;

        match self {
            And(expr) => {
                fmt!(f, Delimited(&expr.operands, " AND "));
            }
            BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                fmt!(f, expr.lhs " " expr.op " " expr.rhs);
            }
            Column(expr) => {
                let column = f.serializer.column_name(expr.column);
                fmt!(f, column);
            }
            InList(expr) => {
                fmt!(f, expr.expr " IN " expr.list);
            }
            InSubquery(expr) => {
                fmt!(f, expr.expr " IN (" expr.query ")");
            }
            IsNull(expr) => {
                if expr.negate {
                    fmt!(f, expr.expr " IS NOT NULL");
                } else {
                    fmt!(f, expr.expr " IS NULL");
                }
            }
            Or(expr) => {
                fmt!(f, Delimited(&expr.operands, " OR "));
            }
            Pattern(stmt::ExprPattern::BeginsWith(expr)) => {
                let stmt::Expr::Value(pattern) = &*expr.pattern else {
                    todo!()
                };

                let pattern = pattern.expect_string();
                let pattern = format!("{pattern}%");
                let pattern = stmt::Expr::Value(pattern.into());

                fmt!(f, expr.expr " LIKE " pattern);
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

impl ToSql for ExprAsList<'_> {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        /*
        match self.0 {
            stmt::Expr::Record(expr) => Comma(&expr.fields).to_sql(f),
            stmt::Expr::List(expr) => Comma(&expr.items).to_sql(f),
            stmt::Expr::Value(stmt::Value::Record(expr)) => self.value_list(expr),
            stmt::Expr::Value(stmt::Value::List(expr)) => self.value_list(expr),
            _ => self.expr(expr),
        }
        */
        todo!()
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
