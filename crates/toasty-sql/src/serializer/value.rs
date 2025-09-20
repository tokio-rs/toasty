use super::{Comma, Params, ToSql};

use crate::{serializer::ExprContext, stmt};

impl ToSql for &stmt::Value {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        use stmt::Value::*;

        match self {
            Id(_) => todo!(),
            Record(value) => fmt!(cx, f, "(" Comma(&value.fields) ")"),
            List(values) => fmt!(cx, f, "(" Comma(values) ")"),
            value => {
                let placeholder = f.params.push(value);
                fmt!(cx, f, placeholder)
            }
        }
    }
}
