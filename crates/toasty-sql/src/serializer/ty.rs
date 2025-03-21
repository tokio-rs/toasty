use super::{Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::Type {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        fmt!(
            f,
            match self {
                stmt::Type::Boolean => "BOOLEAN",
                stmt::Type::Integer => "INTEGER",
                stmt::Type::Text => "TEXT",
            }
        );
    }
}
