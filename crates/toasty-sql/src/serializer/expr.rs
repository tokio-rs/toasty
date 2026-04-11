use toasty_core::stmt::ResolvedRef;

use super::{ColumnAlias, Comma, Delimited, Ident, Params, ToSql};

use crate::{
    serializer::{ExprContext, Flavor},
    stmt,
};

use toasty_core::schema::db;

/// Wrapper for serializing a field within an INSERT VALUES record with type hints
struct TypeHintedField<'a> {
    field_index: usize,
    expr: &'a stmt::Expr,
}

impl<'a> ToSql for TypeHintedField<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Get type hint and storage type from insert context if available
        let (type_hint, storage_ty) = f
            .insert_context
            .as_ref()
            .and_then(|insert_ctx| {
                if self.field_index < insert_ctx.columns.len()
                    && !matches!(self.expr, stmt::Expr::Default)
                {
                    let col_id = insert_ctx.columns[self.field_index];
                    let table = &cx.schema().tables[insert_ctx.table_id.0];
                    let col = &table.columns[col_id.index];
                    Some((Some(col.ty.clone()), Some(col.storage_ty.clone())))
                } else {
                    None
                }
            })
            .unwrap_or((None, None));

        // If this is a Value expr with a type hint, serialize with the hint
        if let (stmt::Expr::Value(value), Some(type_hint)) = (self.expr, type_hint) {
            let mut placeholder = f.params.push(value, Some(&type_hint));
            // PostgreSQL native enums need a cast from TEXT to the enum type
            if matches!(f.serializer.flavor, Flavor::Postgresql) {
                if let Some(db::Type::Enum(ref type_enum)) = storage_ty {
                    if let Some(ref name) = type_enum.name {
                        placeholder.cast = Some(name.clone());
                    }
                }
            }
            fmt!(cx, f, placeholder);
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
            stmt::Expr::Ident(name) => {
                fmt!(cx, f, Ident(name));
            }
            stmt::Expr::InList(expr) => {
                fmt!(cx, f, expr.expr " IN " expr.list);
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
