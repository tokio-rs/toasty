use super::{Comma, Delimited, Params, ToSql};

use crate::stmt;
use toasty_core::schema::db::ColumnId;

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
            Column(expr_column) => {
                // Check if we're in a DDL context (CREATE TABLE/INDEX) where we should use plain column names
                // DDL statements have depth 0 and don't use table aliases
                let is_ddl_context = f.depth == 0;

                if is_ddl_context || (expr_column.nesting == 0 && expr_column.table == 0) {
                    // In DDL context or simple column references, use just the column name
                    let table_idx = if is_ddl_context {
                        // In DDL context, use the table index from the column reference
                        expr_column.table
                    } else {
                        // For simple references, try table 0 first
                        0
                    };

                    let mut found_column = false;

                    // Try the specified table first
                    if let Some(table) = f.serializer.schema.tables.get(table_idx) {
                        if expr_column.column < table.columns.len() {
                            let reconstructed_column_id = ColumnId {
                                table: toasty_core::schema::db::TableId(table_idx),
                                index: expr_column.column,
                            };
                            let column_name = f.serializer.column_name(reconstructed_column_id);
                            fmt!(f, column_name);
                            found_column = true;
                        }
                    }

                    // If that didn't work and we're not in DDL context, try other tables
                    if !found_column && !is_ddl_context {
                        for (other_table_idx, table) in
                            f.serializer.schema.tables.iter().enumerate()
                        {
                            if other_table_idx != table_idx
                                && expr_column.column < table.columns.len()
                            {
                                let reconstructed_column_id = ColumnId {
                                    table: toasty_core::schema::db::TableId(other_table_idx),
                                    index: expr_column.column,
                                };
                                let column_name = f.serializer.column_name(reconstructed_column_id);
                                fmt!(f, column_name);
                                found_column = true;
                                break;
                            }
                        }
                    }

                    if !found_column {
                        // Fallback if no table found with that column index
                        fmt!(f, "col_" expr_column.column)
                    }
                } else {
                    let depth = f.depth - expr_column.nesting;

                    // TODO: Need to determine if the table is a CTE or regular table to use the correct prefix
                    // (cte_ vs tbl_). For now, assuming tbl_ prefix until we can access SourceTable context.
                    // This needs to be fixed to match the aliasing used in the FROM clause.
                    fmt!(f, "tbl_" depth "_" expr_column.table ".col_" expr_column.column)
                }
            }
            Func(stmt::ExprFunc::Count(func)) => match (&func.arg, &func.filter) {
                (None, None) => fmt!(f, "COUNT(*)"),
                // Mysql does not support filters, so translate it to an expression
                (None, Some(expr)) if f.serializer.is_mysql() => {
                    fmt!(f, "COUNT(CASE WHEN " expr " THEN 1 END)")
                }
                (None, Some(expr)) => fmt!(f, "COUNT(*) FILTER (WHERE " expr ")"),
                _ => todo!("func={func:#?}"),
            },
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
            Stmt(expr) => {
                let stmt = &*expr.stmt;
                fmt!(f, "(" stmt ")");
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
