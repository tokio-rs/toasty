use super::{Params, ToSql};

use crate::stmt;

impl ToSql for &stmt::ColumnType {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::ColumnType::Boolean => fmt!(f, "BOOLEAN"),
            stmt::ColumnType::Integer => fmt!(f, "INTEGER"),
            stmt::ColumnType::Text => fmt!(f, "TEXT"),
            stmt::ColumnType::VarChar(size) => fmt!(f, "VARCHAR(" size ")"),
        }
    }
}
