use super::{Comma, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::Value {
    fn to_sql(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_>) {
        use stmt::Value::*;

        match self {
            Record(value) => {
                let fields = Comma(value.fields.iter());
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
                f.dst.push_str("NULL");
            }
            // Inline as a SQL literal (used in DDL contexts like CHECK constraints).
            // DML values are already extracted as Expr::Arg placeholders before
            // reaching the serializer.
            String(s) => {
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
            I64(n) => {
                use std::fmt::Write;
                write!(f.dst, "{n}").unwrap();
            }
            Bool(b) => {
                f.dst.push_str(if *b { "TRUE" } else { "FALSE" });
            }
            _ => todo!("inline SQL literal for value: {self:?}"),
        }
    }
}
