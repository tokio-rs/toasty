use toasty_core::stmt::ResolvedRef;

use super::{ColumnAlias, Comma, Delimited, Ident, Params, ToSql};

use crate::{
    serializer::{ExprContext, Flavor},
    stmt,
};

/// Wrapper for serializing a field within an INSERT VALUES record with type hints
struct TypeHintedField<'a> {
    field_index: usize,
    expr: &'a stmt::Expr,
}

impl<'a> ToSql for TypeHintedField<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Skip type hint for DEFAULT expressions — they don't need one.
        let col = if matches!(self.expr, stmt::Expr::Default) {
            None
        } else {
            f.insert_column(self.field_index, cx.schema())
        };

        // If this is a non-null Value expr with column context, serialize as a
        // bind parameter. NULL is always inlined as a literal.
        if let (stmt::Expr::Value(value), Some(col)) = (self.expr, col) {
            if matches!(value, stmt::Value::Null) {
                f.dst.push_str("NULL");
            } else {
                let placeholder = f.params.push(value, Some(&col.storage_ty));
                fmt!(cx, f, placeholder);
            }
        } else {
            // Other expr types (including Default) serialize normally
            self.expr.to_sql(cx, f);
        }
    }
}

impl ToSql for &stmt::Expr {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Expr::And(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " AND "));
            }
            stmt::Expr::BinaryOp(expr) => {
                assert!(!expr.lhs.is_value_null());
                assert!(!expr.rhs.is_value_null());

                // When one side is a column reference and the other is a value,
                // pass the column's storage type so the driver can bind the
                // parameter with the correct type (e.g. native enum OID).
                let lhs_col = f.column_for_ref(&expr.lhs, cx);
                let rhs_col = f.column_for_ref(&expr.rhs, cx);

                if let (Some(col), stmt::Expr::Value(value)) = (rhs_col, &*expr.lhs) {
                    // RHS is column, LHS is value → bind LHS with RHS column info
                    let placeholder = f.params.push(value, Some(&col.storage_ty));
                    fmt!(cx, f, placeholder " " expr.op " " expr.rhs);
                } else if let (Some(col), stmt::Expr::Value(value)) = (lhs_col, &*expr.rhs) {
                    // LHS is column, RHS is value → bind RHS with LHS column info
                    let placeholder = f.params.push(value, Some(&col.storage_ty));
                    fmt!(cx, f, expr.lhs " " expr.op " " placeholder);
                } else {
                    fmt!(cx, f, expr.lhs " " expr.op " " expr.rhs);
                }
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
            stmt::Expr::Ident(name) => {
                fmt!(cx, f, Ident(name));
            }
            stmt::Expr::InList(expr) => {
                // When the expression is a column reference, pass storage type
                // info for each list item so the driver can bind correctly.
                let col = f.column_for_ref(&expr.expr, cx);
                if let Some(col) = col {
                    fmt!(cx, f, expr.expr " IN ");
                    // Serialize list items with column type info
                    serialize_list_with_storage_ty(cx, f, &expr.list, col);
                } else {
                    fmt!(cx, f, expr.expr " IN " expr.list);
                }
            }
            stmt::Expr::InSubquery(expr) => {
                fmt!(cx, f, expr.expr " IN (" expr.query ")");
            }
            stmt::Expr::IsNull(expr) => {
                fmt!(cx, f, expr.expr " IS NULL");
            }
            stmt::Expr::Not(expr) => {
                fmt!(cx, f, "NOT (" expr.expr ")");
            }
            stmt::Expr::Or(expr) => {
                fmt!(cx, f, Delimited(&expr.operands, " OR "));
            }
            stmt::Expr::Record(expr) => {
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

/// Serializes a list expression, pushing each value item with the given
/// column's storage type info.
fn serialize_list_with_storage_ty<P: Params>(
    cx: &ExprContext<'_>,
    f: &mut super::Formatter<'_, P>,
    list: &stmt::Expr,
    col: &toasty_core::schema::db::Column,
) {
    match list {
        stmt::Expr::List(list) => {
            f.dst.push('(');
            for (i, item) in list.items.iter().enumerate() {
                if i > 0 {
                    f.dst.push_str(", ");
                }
                if let stmt::Expr::Value(value) = item {
                    let placeholder = f.params.push(value, Some(&col.storage_ty));
                    fmt!(cx, f, placeholder);
                } else {
                    item.to_sql(cx, f);
                }
            }
            f.dst.push(')');
        }
        stmt::Expr::Value(stmt::Value::List(values)) => {
            f.dst.push('(');
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    f.dst.push_str(", ");
                }
                let placeholder = f.params.push(value, Some(&col.storage_ty));
                fmt!(cx, f, placeholder);
            }
            f.dst.push(')');
        }
        other => other.to_sql(cx, f),
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
        })
    }
}
