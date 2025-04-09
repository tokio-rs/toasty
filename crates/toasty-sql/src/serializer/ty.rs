use super::{Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::Type {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
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
