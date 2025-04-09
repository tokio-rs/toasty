use super::{Comma, Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::Value {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        use stmt::Value::*;

        match self {
            Id(_) => todo!(),
            Record(value) => fmt!(f, "(" Comma(&value.fields) ")"),
            List(values) => fmt!(f, "(" Comma(values) ")"),
            value => {
                let placeholder = f.params.push(value);
                fmt!(f, placeholder)
            }
        }
    }
}
