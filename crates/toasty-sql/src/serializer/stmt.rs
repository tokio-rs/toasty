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

impl ToSql for &stmt::Insert {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        fmt!(
            f, "INSERT INTO " self.target " " self.source self.returning ";"
        );
    }
}

impl ToSql for &stmt::InsertTarget {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        match self {
            stmt::InsertTarget::Table(insert_table) => {
                let table_name = f.serializer.table_name(insert_table);
                let columns = Comma(
                    insert_table
                        .columns
                        .iter()
                        .map(|column_id| f.serializer.column_name(*column_id)),
                );

                fmt!(f, table_name " (" columns ")");
            }
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::Query {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        match &*self.body {
            stmt::ExprSet::Select(_) => todo!(),
            stmt::ExprSet::Values(values) => values.to_sql(f),
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &Option<stmt::Returning> {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        if let Some(returning) = self {
            // fmt!(f, "RETURNING " returning);
            todo!()
        }
    }
}

impl ToSql for &stmt::Values {
    fn to_sql<T: Params>(self, f: &mut super::Formatter<'_, T>) {
        let rows = Comma(self.rows.iter());

        fmt!(f, "VALUES " rows)
    }
}
