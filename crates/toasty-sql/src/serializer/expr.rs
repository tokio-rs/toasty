use super::{Comma, Delimited, Params, ToSql};

use crate::{
    serializer::{ExprContext, Ident},
    stmt,
};

impl ToSql for &stmt::Expr {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        use stmt::Expr::*;

        match self {
            And(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " AND "));
            }
            BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                fmt!(cx, f, expr.lhs " " expr.op " " expr.rhs);
            }
            Column(expr_column) if f.ddl => {
                let column = cx.resolve_expr_column(expr_column);
                fmt!(cx, f, Ident(&column.name))
            }
            Column(stmt::ExprColumn {
                nesting,
                table,
                column,
            }) => {
                let depth = f.depth - *nesting;
                fmt!(cx, f, "tbl_" depth "_" table ".col_" column)
            }
            Func(stmt::ExprFunc::Count(func)) => match (&func.arg, &func.filter) {
                (None, None) => fmt!(cx, f, "COUNT(*)"),
                // Mysql does not support filters, so translate it to an expression
                (None, Some(expr)) if f.serializer.is_mysql() => {
                    fmt!(cx, f, "COUNT(CASE WHEN " expr " THEN 1 END)")
                }
                (None, Some(expr)) => fmt!(cx, f, "COUNT(*) FILTER (WHERE " expr ")"),
                _ => todo!("func={func:#?}"),
            },
            InList(expr) => {
                fmt!(cx, f, expr.expr " IN " expr.list);
            }
            InSubquery(expr) => {
                fmt!(cx, f, expr.expr " IN (" expr.query ")");
            }
            IsNull(expr) => {
                if expr.negate {
                    fmt!(cx, f, expr.expr " IS NOT NULL");
                } else {
                    fmt!(cx, f, expr.expr " IS NULL");
                }
            }
            Or(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " OR "));
            }
            Pattern(stmt::ExprPattern::BeginsWith(expr)) => {
                let stmt::Expr::Value(pattern) = &*expr.pattern else {
                    todo!()
                };

                let pattern = pattern.expect_string();
                let pattern = format!("{pattern}%");
                let pattern = stmt::Expr::Value(pattern.into());

                fmt!(cx, f, expr.expr " LIKE " pattern);
            }
            Record(expr) => {
                let exprs = Comma(&expr.fields);
                fmt!(cx, f, "(" exprs ")");
            }
            Stmt(expr) => {
                let stmt = &*expr.stmt;
                fmt!(cx, f, "(" stmt ")");
            }
            Value(expr) => expr.to_sql(cx, f),
            _ => todo!("expr={:?}", self),
        }
    }
}

impl ToSql for &stmt::BinaryOp {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
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
