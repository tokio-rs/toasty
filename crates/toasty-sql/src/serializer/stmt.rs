use super::{Comma, Delimited, Ident, Params, ToSql};

use crate::stmt;
use toasty_core::schema::db;

struct ColumnsWithConstraints<'a>(&'a stmt::CreateTable);

impl ToSql for ColumnsWithConstraints<'_> {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let columns = Comma(&self.0.columns);

        if let Some(pk) = &self.0.primary_key {
            fmt!(f, columns ", PRIMARY KEY " pk);
        } else {
            fmt!(f, columns);
        }
    }
}

impl ToSql for &stmt::CreateIndex {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let table_name = f.serializer.table_name(self.on);
        let columns = Comma(&self.columns);
        let unique = if self.unique { "UNIQUE " } else { "" };

        fmt!(
            f, "CREATE " unique "INDEX " self.name " ON " table_name " (" columns ");"
        );
    }
}

impl ToSql for &stmt::CreateTable {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let columns = ColumnsWithConstraints(self);

        fmt!(
            f, "CREATE TABLE " self.name " (" columns ");"
        );
    }
}

impl ToSql for &stmt::Delete {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        assert!(self.returning.is_none());

        fmt!(f, "DELETE FROM " self.from " WHERE " self.filter ";");
    }
}

impl ToSql for &stmt::DropTable {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let if_exists = if self.if_exists { "IF EXISTS " } else { "" };
        fmt!(f, "DROP TABLE " if_exists self.name ";");
    }
}

impl ToSql for &stmt::Insert {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let returning = self
            .returning
            .as_ref()
            .map(|returning| ("RETURNING ", returning));

        fmt!(
            f, "INSERT INTO " self.target " " self.source returning ";"
        );
    }
}

impl ToSql for &stmt::InsertTarget {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
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
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match &*self.body {
            stmt::ExprSet::Select(stmt) => stmt.to_sql(f),
            stmt::ExprSet::Values(values) => values.to_sql(f),
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::Returning {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Returning::Star => fmt!(f, "*"),
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                fmt!(f, Comma(&expr_record.fields));
            }
            stmt::Returning::Expr(expr) => {
                fmt!(f, expr);
            }
            _ => todo!(),
        }
    }
}

impl ToSql for &stmt::Select {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        fmt!(
            f,
            "SELECT " self.returning " FROM " self.source
            " WHERE " self.filter
        );
    }
}

impl ToSql for &stmt::Source {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Source::Table(table) => {
                fmt!(f, Comma(table));
            }
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::TableWithJoins {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let table_name = f.serializer.table_name(self.table);
        fmt!(f, table_name);
    }
}

impl ToSql for &stmt::Update {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.schema.table(self.target.as_table().table);
        let assignments = (table, &self.assignments);
        let filter = self.filter.as_ref().map(|expr| (" WHERE ", expr));
        let returning = self
            .returning
            .as_ref()
            .map(|returning| (" RETURNING ", returning));

        assert!(
            self.condition.is_none(),
            "SQL does not support update conditions"
        );

        fmt!(f, "UPDATE " self.target " SET " assignments filter returning ";");
    }
}

impl ToSql for (&db::Table, &stmt::Assignments) {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let frags = self.1.iter().map(|(index, assignment)| {
            let column_name = Ident(&self.0.columns[index].name);
            (column_name, " = ", &assignment.expr)
        });

        fmt!(f, Delimited(frags, " "));
    }
}

impl ToSql for &stmt::UpdateTarget {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::UpdateTarget::Table(table_with_joins) => table_with_joins.to_sql(f),
            _ => todo!(),
        }
    }
}

impl ToSql for &stmt::Values {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let rows = Comma(self.rows.iter());

        fmt!(f, "VALUES " rows)
    }
}
