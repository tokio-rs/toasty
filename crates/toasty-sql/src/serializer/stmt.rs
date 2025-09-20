use std::mem;

use super::{Comma, Delimited, Ident, Params, ToSql};

use crate::{serializer::ExprContext, stmt};
use toasty_core::{schema::db, stmt::SourceTableId};

struct ColumnsWithConstraints<'a>(&'a stmt::CreateTable);

impl ToSql for ColumnsWithConstraints<'_> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let columns = Comma(&self.0.columns);

        if let Some(pk) = &self.0.primary_key {
            fmt!(cx, f, columns ", PRIMARY KEY " pk);
        } else {
            fmt!(cx, f, columns);
        }
    }
}

impl ToSql for &stmt::CreateIndex {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let index = f.serializer.index(self.index);
        let table = f.serializer.table(self.on);
        let index_name = Ident(&index.name);
        let table_name = Ident(&table.name);
        let columns = Comma(&self.columns);
        let unique = if self.unique { "UNIQUE " } else { "" };

        // Create a new expression scope to serialize the statement
        let cx = cx.scope(table);

        fmt!(
            &cx, f, "CREATE " unique "INDEX " index_name " ON " table_name " (" columns ")"
        );
    }
}

impl ToSql for &stmt::CreateTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.table(self.table);
        let name = Ident(&table.name);
        let columns = ColumnsWithConstraints(self);

        // Create new expression scope to serialize the statement
        let cx = cx.scope(table);

        fmt!(
            &cx, f, "CREATE TABLE " name " (" columns ")"
        );
    }
}

impl ToSql for &stmt::Delete {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let prev = mem::replace(&mut f.alias, true);

        assert!(self.returning.is_none());

        // Create a new expression scope to serialize the statement
        let cx = cx.scope(self);

        fmt!(&cx, f, "DELETE FROM " self.from " WHERE " self.filter);

        f.alias = prev;
    }
}

impl ToSql for &stmt::Direction {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Direction::Asc => fmt!(cx, f, "ASC"),
            stmt::Direction::Desc => fmt!(cx, f, "DESC"),
        }
    }
}

impl ToSql for &stmt::DropTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let if_exists = if self.if_exists { "IF EXISTS " } else { "" };
        fmt!(cx, f, "DROP TABLE " if_exists self.name);
    }
}

impl ToSql for &stmt::Insert {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Create a new expression scope to serialize the statement
        let cx = cx.scope(self);

        let returning = self
            .returning
            .as_ref()
            .map(|returning| ("RETURNING ", returning));

        fmt!(
            &cx, f, "INSERT INTO " self.target " " self.source returning
        );
    }
}

impl ToSql for &stmt::InsertTarget {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::InsertTarget::Table(insert_table) => {
                let table_name = f.serializer.table_name(insert_table);
                let columns = Comma(
                    insert_table
                        .columns
                        .iter()
                        .map(|column_id| f.serializer.column_name(*column_id)),
                );

                fmt!(cx, f, table_name " (" columns ")");
            }
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::Limit {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        assert!(self.offset.is_none(), "TODO");

        fmt!(cx, f, "LIMIT " self.limit);
    }
}

impl ToSql for &stmt::Query {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let prev = mem::replace(&mut f.alias, true);
        // Create a new expression scope to serialize the statement
        let cx = cx.scope(self);

        let locks = if self.locks.is_empty() {
            None
        } else {
            Some((" ", Delimited(&self.locks, " ")))
        };

        let body = &self.body;
        let order_by = self.order_by.as_ref().map(|order_by| (" ", order_by));
        let limit = self.limit.as_ref().map(|limit| (" ", limit));

        fmt!(&cx, f, self.with body order_by limit locks);

        f.alias = prev;
    }
}

impl ToSql for &stmt::ExprSet {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::ExprSet::Select(expr) => expr.to_sql(cx, f),
            stmt::ExprSet::Values(expr) => expr.to_sql(cx, f),
            stmt::ExprSet::Update(expr) => expr.to_sql(cx, f),
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &stmt::OrderBy {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let order_by = Comma(&self.exprs);

        fmt!(cx, f, "ORDER BY " order_by);
    }
}

impl ToSql for &stmt::OrderByExpr {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        if let Some(order) = &self.order {
            fmt!(cx, f, self.expr " " order);
        } else {
            fmt!(cx, f, self.expr);
        }
    }
}

impl ToSql for &stmt::Returning {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Returning::Model { .. } => fmt!(cx, f, "*"),
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                let fields = expr_record
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, expr)| match expr {
                        stmt::Expr::Column(_) => (expr, None, None),
                        _ => (expr, Some(" AS col_"), Some(i)),
                    });

                fmt!(cx, f, Comma(fields));
            }
            stmt::Returning::Expr(expr) => {
                fmt!(cx, f, expr);
            }
            _ => todo!(),
        }
    }
}

impl ToSql for &stmt::Select {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        fmt!(
            cx, f,
            "SELECT " self.returning " FROM " self.source
            " WHERE " self.filter
        );
    }
}

impl ToSql for &stmt::Lock {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Lock::Update => fmt!(cx, f, "FOR UPDATE"),
            stmt::Lock::Share => fmt!(cx, f, "FOR SHARE"),
        }
    }
}

impl ToSql for &stmt::Source {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Source::Table(source_table) => {
                source_table.to_sql(cx, f);
            }
            _ => todo!("self={self:?}"),
        }
    }
}

impl ToSql for &toasty_core::stmt::Statement {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        use toasty_core::stmt::Statement::*;

        f.depth += 1;

        match self {
            Delete(stmt) => stmt.to_sql(cx, f),
            Insert(stmt) => stmt.to_sql(cx, f),
            Query(stmt) => stmt.to_sql(cx, f),
            Update(stmt) => stmt.to_sql(cx, f),
        }

        f.depth -= 1;
    }
}

impl ToSql for &stmt::Statement {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::Statement::CreateIndex(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::CreateTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::DropTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Delete(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Insert(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Query(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Update(stmt) => stmt.to_sql(cx, f),
        }
    }
}

impl ToSql for &stmt::SourceTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Serialize the main table relation
        match &self.from_item.relation {
            stmt::TableFactor::Table(table_id) => {
                let table_ref = &self.tables[table_id.0];
                let alias = TableAlias {
                    depth: f.depth,
                    table: *table_id,
                };

                fmt!(cx, f, table_ref " AS " alias);
            }
        }

        // Serialize the joins
        for join in &self.from_item.joins {
            match &join.constraint {
                stmt::JoinOp::Left(expr) => {
                    let join_table_ref = &self.tables[join.table.0];
                    let alias = TableAlias {
                        depth: f.depth,
                        table: join.table,
                    };
                    fmt!(cx, f, " LEFT JOIN " join_table_ref " AS " alias " ON " expr);
                }
            }
        }
    }
}

impl ToSql for &stmt::TableRef {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match *self {
            stmt::TableRef::Table(table_id) => {
                let table_name = f.serializer.table_name(table_id);
                fmt!(cx, f, table_name);
            }
            stmt::TableRef::Cte { nesting, index } => {
                assert!(f.depth >= nesting, "nesting={nesting} depth={}", f.depth);

                let depth = f.depth - nesting;
                fmt!(cx, f, "cte_" depth "_" index);
            }
        }
    }
}

struct TableAlias {
    depth: usize,
    table: SourceTableId,
}

impl ToSql for &TableAlias {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        fmt!(cx, f, "tbl_" self.depth "_" self.table.0);
    }
}

impl ToSql for &stmt::Update {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let prev = mem::replace(&mut f.alias, true);

        let table = f.serializer.schema.table(self.target.as_table());
        let assignments = (table, &self.assignments);

        // Create a new expression scope to serialize the statement
        let cx = cx.scope(self);

        let filter = self.filter.as_ref().map(|expr| (" WHERE ", expr));
        let returning = self
            .returning
            .as_ref()
            .map(|returning| (" RETURNING ", returning));

        assert!(
            self.condition.is_none(),
            "SQL does not support update conditions"
        );

        fmt!(&cx, f, "UPDATE " self.target " SET " assignments filter returning);

        f.alias = prev;
    }
}

impl ToSql for (&db::Table, &stmt::Assignments) {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let frags = self.1.iter().map(|(index, assignment)| {
            let column_name = Ident(&self.0.columns[index].name);
            (column_name, " = ", &assignment.expr)
        });

        fmt!(cx, f, Delimited(frags, ", "));
    }
}

impl ToSql for &stmt::UpdateTarget {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::UpdateTarget::Table(table_id) => {
                let table_name = f.serializer.table_name(*table_id);
                let alias = TableAlias {
                    depth: f.depth,
                    table: SourceTableId(0),
                };

                fmt!(cx, f, table_name " AS " alias);
            }
            _ => todo!(),
        }
    }
}

impl ToSql for &stmt::Values {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let rows = Comma(self.rows.iter());

        fmt!(cx, f, "VALUES " rows)
    }
}
