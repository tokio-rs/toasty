use toasty_core::stmt::ResolvedRef;

use super::{ColumnAlias, Comma, Delimited, Ident, ToSql};

use crate::{
    serializer::{ExprContext, Flavor},
    stmt,
};

impl ToSql for &stmt::Expr {
    fn to_sql(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_>) {
        match self {
            stmt::Expr::And(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " AND "));
            }
            stmt::Expr::BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                fmt!(cx, f, expr.lhs " " expr.op " " expr.rhs);
            }
            stmt::Expr::Exists(expr) => {
                f.depth += 1;
                fmt!(cx, f, "EXISTS (" expr.subquery ")");
                f.depth -= 1;
            }
            stmt::Expr::Func(stmt::ExprFunc::Count(func)) => match (&func.arg, &func.filter) {
                (None, None) => fmt!(cx, f, "COUNT(*)"),
                // Mysql does not support filters, so translate it to an expression
                (None, Some(expr)) if f.serializer.is_mysql() => {
                    fmt!(cx, f, "COUNT(CASE WHEN " expr " THEN 1 END)")
                }
                (None, Some(expr)) => fmt!(cx, f, "COUNT(*) FILTER (WHERE " expr ")"),
                _ => todo!("func={func:#?}"),
            },
            stmt::Expr::Func(stmt::ExprFunc::LastInsertId(_)) => {
                fmt!(cx, f, "LAST_INSERT_ID()")
            }
            stmt::Expr::IsSuperset(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, e.lhs.as_ref() " @> " e.rhs.as_ref()),
                Flavor::Mysql | Flavor::Sqlite => unreachable!(
                    "is_superset on a native array column requires PostgreSQL; schema build \
                     rejects `Vec<T>` fields on this backend"
                ),
            },
            stmt::Expr::Intersects(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, e.lhs.as_ref() " && " e.rhs.as_ref()),
                Flavor::Mysql | Flavor::Sqlite => unreachable!(
                    "intersects on a native array column requires PostgreSQL; schema build \
                     rejects `Vec<T>` fields on this backend"
                ),
            },
            stmt::Expr::Length(e) => match f.serializer.flavor {
                Flavor::Postgresql => fmt!(cx, f, "cardinality(" e.expr.as_ref() ")"),
                Flavor::Mysql | Flavor::Sqlite => unreachable!(
                    "array length on a native array column requires PostgreSQL; schema build \
                     rejects `Vec<T>` fields on this backend"
                ),
            },
            stmt::Expr::Ident(name) => {
                fmt!(cx, f, Ident(name));
            }
            stmt::Expr::InList(expr) => {
                fmt!(cx, f, expr.expr " IN " expr.list);
            }
            stmt::Expr::AnyOp(expr) => {
                fmt!(cx, f, expr.lhs " " expr.op " ANY(" expr.rhs ")");
            }
            stmt::Expr::AllOp(expr) => {
                fmt!(cx, f, expr.lhs " " expr.op " ALL(" expr.rhs ")");
            }
            stmt::Expr::InSubquery(expr) => {
                fmt!(cx, f, expr.expr " IN (" expr.query ")");
            }
            stmt::Expr::IsNull(expr) => {
                fmt!(cx, f, expr.expr " IS NULL");
            }
            stmt::Expr::Like(expr) => {
                let op =
                    if expr.case_insensitive && matches!(f.serializer.flavor, Flavor::Postgresql) {
                        " ILIKE "
                    } else {
                        " LIKE "
                    };
                fmt!(cx, f, expr.expr op expr.pattern);
                if let Some(escape) = expr.escape {
                    let escape = &stmt::Value::String(escape.to_string());
                    fmt!(cx, f, " ESCAPE " escape);
                }
            }
            stmt::Expr::StartsWith(expr) => {
                // The lowering pass leaves `StartsWith` in place when
                // `Capability::native_prefix_match_op` is true. PostgreSQL
                // is the only such SQL flavor today (`^@` operator).
                match f.serializer.flavor {
                    Flavor::Postgresql => {
                        fmt!(cx, f, expr.expr " ^@ " expr.prefix);
                    }
                    Flavor::Sqlite | Flavor::Mysql => {
                        unreachable!(
                            "StartsWith should have been lowered to LIKE for non-PostgreSQL flavors"
                        );
                    }
                }
            }
            stmt::Expr::Not(expr) => {
                fmt!(cx, f, "NOT (" expr.expr ")");
            }
            stmt::Expr::Or(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " OR "));
            }
            stmt::Expr::Record(expr) => {
                let fields = Comma(expr.fields.iter());
                fmt!(cx, f, "(" fields ")");
            }
            stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Column(expr_column)) => {
                if f.alias {
                    let depth = f.depth - expr_column.nesting;

                    match cx.resolve_expr_reference(expr_reference) {
                        ResolvedRef::Column(column) => {
                            let name = Ident(&column.name);
                            fmt!(cx, f, "tbl_" depth "_" expr_column.table "." name)
                        }
                        ResolvedRef::Cte { .. } | ResolvedRef::Derived(_) => {
                            fmt!(cx, f, "tbl_" depth "_" expr_column.table "." ColumnAlias(expr_column.column))
                        }
                        ResolvedRef::Model(model) => {
                            panic!("Model references cannot be serialized to SQL; model={model:?}")
                        }
                        ResolvedRef::Field(field) => {
                            panic!("Field references cannot be serialized to SQL; field={field:?}")
                        }
                    }
                } else {
                    let column = cx.resolve_expr_reference(expr_reference).as_column_unwrap();
                    fmt!(cx, f, Ident(&column.name))
                }
            }
            stmt::Expr::Stmt(expr) => {
                let stmt = &*expr.stmt;
                fmt!(cx, f, "(" stmt ")");
            }
            stmt::Expr::List(expr) => {
                let items = Comma(expr.items.iter());
                fmt!(cx, f, "(" items ")");
            }
            stmt::Expr::Value(expr) => expr.to_sql(cx, f),
            stmt::Expr::Arg(arg) => {
                // Pre-extracted bind parameter placeholder — render as a
                // positional parameter. The arg position is 0-based; the
                // placeholder is 1-based.
                f.arg_positions.push(arg.position);
                let placeholder = super::Placeholder(arg.position + 1);
                fmt!(cx, f, placeholder);
            }
            stmt::Expr::Default => match f.serializer.flavor {
                Flavor::Postgresql | Flavor::Mysql => fmt!(cx, f, "DEFAULT"),
                // SQLite does not support the DEFAULT keyword but NULL acts similarly.
                Flavor::Sqlite => fmt!(cx, f, "NULL"),
            },
            _ => todo!("expr={:#?}", self),
        }
    }
}

impl ToSql for &stmt::BinaryOp {
    fn to_sql(self, _cx: &ExprContext<'_>, f: &mut super::Formatter<'_>) {
        f.dst.push_str(match self {
            stmt::BinaryOp::Eq => "=",
            stmt::BinaryOp::Gt => ">",
            stmt::BinaryOp::Ge => ">=",
            stmt::BinaryOp::Lt => "<",
            stmt::BinaryOp::Le => "<=",
            stmt::BinaryOp::Ne => "<>",
        })
    }
}
