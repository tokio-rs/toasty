use toasty_core::stmt::ResolvedRef;

use super::{ColumnAlias, Comma, Delimited, Params, ToSql};

use crate::{
    serializer::{ExprContext, Flavor, Ident},
    stmt,
};

/// Wrapper for serializing a field within an INSERT VALUES record with type hints
struct TypeHintedField<'a> {
    field_index: usize,
    expr: &'a stmt::Expr,
}

impl<'a> ToSql for TypeHintedField<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Get type hint from insert context if available
        let type_hint = f.insert_context.as_ref().and_then(|insert_ctx| {
            if self.field_index < insert_ctx.columns.len()
                && !matches!(self.expr, stmt::Expr::Default)
            {
                let col_id = insert_ctx.columns[self.field_index];
                let table = &cx.schema().tables[insert_ctx.table_id.0];
                Some(table.columns[col_id.index].ty.clone())
            } else {
                None
            }
        });

        // If this is a Value expr with a type hint, serialize with the hint
        if let (stmt::Expr::Value(value), Some(type_hint)) = (self.expr, type_hint) {
            let placeholder = f.params.push(value, Some(&type_hint));
            fmt!(cx, f, placeholder);
        } else {
            // Other expr types (including Default) serialize normally
            self.expr.to_sql(cx, f);
        }
    }
}

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
            Exists(expr) => {
                f.depth += 1;
                fmt!(cx, f, "EXISTS (" expr.subquery ")");
                f.depth -= 1;
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
            Func(stmt::ExprFunc::LastInsertId(_)) => {
                fmt!(cx, f, "LAST_INSERT_ID()")
            }
            InList(expr) => {
                fmt!(cx, f, expr.expr " IN " expr.list);
            }
            InSubquery(expr) => {
                fmt!(cx, f, expr.expr " IN (" expr.query ")");
            }
            IsNull(expr) => {
                fmt!(cx, f, expr.expr " IS NULL");
            }
            Not(expr) => {
                fmt!(cx, f, "NOT (" expr.expr ")");
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
                // Use TypeHintedField wrapper to provide type hints from INSERT context
                let fields =
                    Comma(
                        expr.fields
                            .iter()
                            .enumerate()
                            .map(|(i, field)| TypeHintedField {
                                field_index: i,
                                expr: field,
                            }),
                    );
                fmt!(cx, f, "(" fields ")");
            }
            Reference(expr_reference @ stmt::ExprReference::Column(expr_column)) => {
                if f.alias {
                    let depth = f.depth - expr_column.nesting;

                    match cx.resolve_expr_reference(expr_reference) {
                        ResolvedRef::Column(column) => {
                            let name = Ident(&column.name);
                            fmt!(cx, f, "tbl_" depth "_" expr_column.table "." name)
                        }
                        ResolvedRef::Cte { .. } | ResolvedRef::Derived { .. } => {
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
                    let column = cx.resolve_expr_reference(expr_reference).expect_column();
                    fmt!(cx, f, Ident(&column.name))
                }
            }
            Stmt(expr) => {
                let stmt = &*expr.stmt;
                fmt!(cx, f, "(" stmt ")");
            }
            Value(expr) => expr.to_sql(cx, f),
            Default => match f.serializer.flavor {
                Flavor::Postgresql | Flavor::Mysql => fmt!(cx, f, "DEFAULT"),
                // SQLite does not support the DEFAULT keyword but NULL acts similarly.
                Flavor::Sqlite => fmt!(cx, f, "NULL"),
            },
            _ => todo!("expr={:#?}", self),
        }
    }
}

impl ToSql for &stmt::BinaryOp {
    fn to_sql<P: Params>(self, _cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
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
