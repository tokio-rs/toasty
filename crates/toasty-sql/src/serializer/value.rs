use super::{Comma, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

/// Wrapper for serializing a value within an INSERT VALUES record with type hints
struct TypeHintedValue<'a> {
    field_index: usize,
    value: &'a stmt::Value,
}

impl<'a> ToSql for TypeHintedValue<'a> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Get type hint from insert context if available
        let type_hint = f.insert_context.as_ref().and_then(|insert_ctx| {
            if self.field_index < insert_ctx.columns.len() {
                let col_id = insert_ctx.columns[self.field_index];
                let table = &cx.schema().tables[insert_ctx.table_id.0];
                Some(table.columns[col_id.index].ty.clone())
            } else {
                None
            }
        });

        // For nested records/lists, recurse normally (they handle their own fields)
        if matches!(self.value, stmt::Value::Record(_) | stmt::Value::List(_)) {
            self.value.to_sql(cx, f);
        } else {
            // For scalar values, use the type hint
            let placeholder = f.params.push(self.value, type_hint.as_ref());
            fmt!(cx, f, placeholder);
        }
    }
}

impl ToSql for &stmt::Value {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        use stmt::Value::*;

        match self {
            Id(_) => todo!(),
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
            value => {
                let placeholder = f.params.push(value, None);
                fmt!(cx, f, placeholder)
            }
        }
    }
}
