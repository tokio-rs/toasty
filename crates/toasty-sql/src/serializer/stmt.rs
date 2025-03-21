use super::{Comma, Params, ToSql};

use crate::stmt;

struct ColumnsWithConstraints<'a>(&'a stmt::CreateTable);

impl ToSql for ColumnsWithConstraints<'_> {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        let columns = Comma(&self.0.columns);

        if let Some(pk) = &self.0.primary_key {
            fmt!(f, columns ", PRIMARY KEY " pk);
        } else {
            fmt!(f, columns);
        }
    }
}

impl ToSql for &stmt::CreateIndex {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        let table_name = f.serializer.table_name(self.on);
        let columns = Comma(&self.columns);

        fmt!(
            f, "CREATE INDEX " self.name " ON " table_name " (" columns ");"
        );
    }
}

impl ToSql for &stmt::CreateTable {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        let columns = ColumnsWithConstraints(self);

        fmt!(
            f, "CREATE TABLE " self.name " (" columns ");"
        );
    }
}
