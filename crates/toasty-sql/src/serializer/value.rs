use super::{Comma, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

/// Wrapper for serializing a value within an INSERT VALUES record with type hints
struct TypeHintedValue<'a> {
    field_index: usize,
    value: &'a stmt::Value,
}

impl<'a> ToSql for TypeHintedValue<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let col = f.insert_column(self.field_index, cx.schema());

        if matches!(self.value, stmt::Value::Null) {
            // Write NULL as a literal — see ToSql for &stmt::Value
            f.dst.push_str("NULL");
        } else if matches!(self.value, stmt::Value::Record(_) | stmt::Value::List(_)) {
            // For nested records/lists, recurse normally (they handle their own fields)
            self.value.to_sql(cx, f);
        } else {
            // For scalar values, pass the column's type hint and storage type
            let placeholder =
                f.params
                    .push(self.value, col.map(|c| &c.ty), col.map(|c| &c.storage_ty));
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
                    let placeholder = f.params.push(value, None, None);
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
