use super::{Comma, Flavor, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

use toasty_core::schema::db;

/// Wrapper for serializing a value within an INSERT VALUES record with type hints
struct TypeHintedValue<'a> {
    field_index: usize,
    value: &'a stmt::Value,
}

impl<'a> ToSql for TypeHintedValue<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Get type hint and storage type from insert context if available
        let (type_hint, storage_ty) = f
            .insert_context
            .as_ref()
            .and_then(|insert_ctx| {
                if self.field_index < insert_ctx.columns.len() {
                    let col_id = insert_ctx.columns[self.field_index];
                    let table = &cx.schema().tables[insert_ctx.table_id.0];
                    let col = &table.columns[col_id.index];
                    Some((Some(col.ty.clone()), col.storage_ty.clone()))
                } else {
                    None
                }
            })
            .unwrap_or((None, db::Type::Text));

        if matches!(self.value, stmt::Value::Null) {
            // Write NULL as a literal — see ToSql for &stmt::Value
            f.dst.push_str("NULL");
        } else if matches!(self.value, stmt::Value::Record(_) | stmt::Value::List(_)) {
            // For nested records/lists, recurse normally (they handle their own fields)
            self.value.to_sql(cx, f);
        } else {
            // For scalar values, use the type hint
            let mut placeholder = f.params.push(self.value, type_hint.as_ref());
            // PostgreSQL native enums need a cast from TEXT to the enum type
            if matches!(f.serializer.flavor, Flavor::Postgresql) {
                if let db::Type::Enum(ref type_enum) = storage_ty {
                    if let Some(ref name) = type_enum.name {
                        placeholder.cast = Some(name.clone());
                    }
                }
            }
            fmt!(cx, f, placeholder);
        }
    }
}

impl ToSql for &stmt::Value {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        use stmt::Value::*;

        match self {
            Record(value) => {
                // Use TypeHintedValue wrapper to provide type hints from INSERT context
                let fields =
                    Comma(
                        value
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(i, field)| TypeHintedValue {
                                field_index: i,
                                value: field,
                            }),
                    );
                fmt!(cx, f, "(" fields ")");
            }
            List(values) => {
                f.dst.push('(');
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        f.dst.push_str(", ");
                    }
                    value.to_sql(cx, f);
                }
                f.dst.push(')');
            }
            Null => {
                // Write NULL as a literal rather than a bind parameter.
                // A bound NULL with no type hint causes PostgreSQL to fail
                // type inference (e.g. in VALUES-based derived tables).
                f.dst.push_str("NULL");
            }
            value => {
                if f.bind_params {
                    let placeholder = f.params.push(value, None);
                    fmt!(cx, f, placeholder)
                } else {
                    // Inline as a SQL literal (used in DDL contexts like CHECK).
                    match value {
                        stmt::Value::String(s) => {
                            f.dst.push('\'');
                            // Escape single quotes by doubling them.
                            for ch in s.chars() {
                                if ch == '\'' {
                                    f.dst.push('\'');
                                }
                                f.dst.push(ch);
                            }
                            f.dst.push('\'');
                        }
                        stmt::Value::I64(n) => {
                            use std::fmt::Write;
                            write!(f.dst, "{n}").unwrap();
                        }
                        stmt::Value::Bool(b) => {
                            f.dst.push_str(if *b { "TRUE" } else { "FALSE" });
                        }
                        _ => todo!("inline SQL literal for value: {value:?}"),
                    }
                }
            }
        }
    }
}
