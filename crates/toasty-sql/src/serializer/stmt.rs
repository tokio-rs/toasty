use super::{Comma, Delimited, Ident, Params, ToSql};

use crate::stmt;
use toasty_core::schema::db;
use toasty_core::stmt::Value;

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
        let unique = if self.unique { "UNIQUE " } else { "" };

        fmt!(f, "CREATE " unique "INDEX " self.name " ON " table_name " (");

        for (i, (column_id, direction)) in self.columns.iter().enumerate() {
            if i > 0 {
                f.dst.push_str(", ");
            }

            // Get column name directly from schema using ColumnId
            let column_name = f.serializer.column_name(*column_id);
            column_name.to_sql(f);

            // Add direction if specified
            if let Some(direction) = direction {
                f.dst.push(' ');
                direction.to_sql(f);
            }
        }

        f.dst.push(')');
    }
}

impl ToSql for &stmt::CreateTable {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let columns = ColumnsWithConstraints(self);

        fmt!(
            f, "CREATE TABLE " self.name " (" columns ")"
        );
    }
}

impl ToSql for &stmt::Delete {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        assert!(self.returning.is_none());

        // Set up table context for column references in WHERE clause
        let tables = match &self.from {
            stmt::Source::Table(table_with_joins) => {
                table_with_joins.iter().map(|twj| twj.table.clone()).collect()
            }
            _ => Vec::new(), // TODO: handle other source types
        };
        f.push_table_context(tables);

        fmt!(f, "DELETE FROM " self.from " WHERE " self.filter);

        f.pop_table_context();
    }
}

impl ToSql for &stmt::Direction {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Direction::Asc => fmt!(f, "ASC"),
            stmt::Direction::Desc => fmt!(f, "DESC"),
        }
    }
}

impl ToSql for &stmt::DropTable {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let if_exists = if self.if_exists { "IF EXISTS " } else { "" };
        fmt!(f, "DROP TABLE " if_exists self.name);
    }
}

impl ToSql for &stmt::Insert {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let returning = self
            .returning
            .as_ref()
            .map(|returning| ("RETURNING ", returning));

        fmt!(
            f, "INSERT INTO " self.target " " self.source returning
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

impl ToSql for &stmt::Limit {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        assert!(self.offset.is_none(), "TODO");

        fmt!(f, "LIMIT " self.limit);
    }
}

impl ToSql for &stmt::Query {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let locks = if self.locks.is_empty() {
            None
        } else {
            Some((" ", Delimited(&self.locks, " ")))
        };

        let body = &self.body;
        let order_by = self.order_by.as_ref().map(|order_by| (" ", order_by));
        let limit = self.limit.as_ref().map(|limit| (" ", limit));

        fmt!(f, self.with body order_by limit locks)
    }
}

impl ToSql for &stmt::ExprSet {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::ExprSet::Select(expr) => expr.to_sql(f),
            stmt::ExprSet::Values(expr) => expr.to_sql(f),
            stmt::ExprSet::Update(expr) => expr.to_sql(f),
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::OrderBy {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let order_by = Comma(&self.exprs);

        fmt!(f, "ORDER BY " order_by);
    }
}

impl ToSql for &stmt::OrderByExpr {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        if let Some(order) = &self.order {
            fmt!(f, self.expr " " order);
        } else {
            fmt!(f, self.expr);
        }
    }
}

impl ToSql for &stmt::Returning {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Returning::Model { .. } => fmt!(f, "*"),
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                let fields = expr_record
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, expr)| match expr {
                        stmt::Expr::Column(_) => (expr, None, None),
                        _ => (expr, Some(" AS col_"), Some(i)),
                    });

                fmt!(f, Comma(fields));
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
        // Extract table references from the source and push to context
        let tables = match &self.source {
            stmt::Source::Table(table_with_joins) => {
                table_with_joins.iter().map(|twj| twj.table.clone()).collect()
            }
            _ => Vec::new(), // TODO: handle other source types
        };

        f.push_table_context(tables);

        fmt!(
            f,
            "SELECT " self.returning " FROM " self.source
            " WHERE " self.filter
        );

        f.pop_table_context();
    }
}

impl ToSql for &stmt::Lock {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Lock::Update => fmt!(f, "FOR UPDATE"),
            stmt::Lock::Share => fmt!(f, "FOR SHARE"),
        }
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

impl ToSql for &toasty_core::stmt::Statement {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        use toasty_core::stmt::Statement::*;

        f.depth += 1;

        match self {
            Delete(stmt) => stmt.to_sql(f),
            Insert(stmt) => stmt.to_sql(f),
            Query(stmt) => stmt.to_sql(f),
            Update(stmt) => stmt.to_sql(f),
        }

        f.depth -= 1;
    }
}

impl ToSql for &stmt::Statement {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Statement::CreateIndex(stmt) => stmt.to_sql(f),
            stmt::Statement::CreateTable(stmt) => stmt.to_sql(f),
            stmt::Statement::DropTable(stmt) => stmt.to_sql(f),
            stmt::Statement::Delete(stmt) => stmt.to_sql(f),
            stmt::Statement::Insert(stmt) => stmt.to_sql(f),
            stmt::Statement::Query(stmt) => stmt.to_sql(f),
            stmt::Statement::Update(stmt) => stmt.to_sql(f),
        }
    }
}

impl ToSql for &stmt::TableWithJoins {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        fmt!(f, &self.table);

        if self.table.is_cte() {
            let depth = f.depth;
            fmt!(f, " AS tbl_" depth);
        }

        for join in &self.joins {
            match &join.constraint {
                stmt::JoinOp::Left(expr) => {
                    fmt!(f, " LEFT JOIN " join.table " ON " expr);
                }
            }
        }
    }
}

impl ToSql for &stmt::TableRef {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match *self {
            stmt::TableRef::Table(table_id) => {
                let table_name = f.serializer.table_name(table_id);
                fmt!(f, table_name);
            }
            stmt::TableRef::Cte { nesting, index } => {
                assert!(f.depth >= nesting, "nesting={nesting} depth={}", f.depth);

                let depth = f.depth - nesting;
                fmt!(f, "cte_" depth "_" index);
            }
        }
    }
}

impl ToSql for &stmt::Update {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.schema.table(self.target.as_table());
        let assignments = (table, &self.assignments);

        // Set up table context for column references in WHERE clause
        let table_ref = stmt::TableRef::Table(self.target.as_table());
        f.push_table_context(vec![table_ref]);

        let filter = self.filter.as_ref().map(|expr| (" WHERE ", expr));
        let returning = self
            .returning
            .as_ref()
            .map(|returning| (" RETURNING ", returning));

        assert!(
            self.condition.is_none(),
            "SQL does not support update conditions"
        );

        fmt!(f, "UPDATE " self.target " SET " assignments filter returning);

        f.pop_table_context();
    }
}

impl ToSql for (&db::Table, &stmt::Assignments) {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        let frags = self.1.iter().map(|(index, assignment)| {
            let column_name = Ident(&self.0.columns[index].name);
            (column_name, " = ", &assignment.expr)
        });

        fmt!(f, Delimited(frags, ", "));
    }
}

impl ToSql for &stmt::UpdateTarget {
    fn to_sql<P: Params>(self, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::UpdateTarget::Table(table_id) => {
                let table_name = f.serializer.table_name(*table_id);
                fmt!(f, table_name);
            }
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
