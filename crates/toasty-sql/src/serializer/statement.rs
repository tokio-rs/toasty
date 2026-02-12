use std::mem;

use super::{ColumnAlias, Comma, Delimited, Ident, Params, ToSql};

use crate::{
    serializer::{ExprContext, Flavor},
    stmt::{self, AlterColumnChanges, ColumnDef},
};
use toasty_core::{schema::db, stmt::SourceTableId};

struct ColumnsWithConstraints<'a>(&'a stmt::CreateTable);

impl ToSql for ColumnsWithConstraints<'_> {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // SQLite needs the PK specified with the auto increment
        let trailing_pk = if f.serializer.is_sqlite() {
            // Sqlite only supports auto incrementing columns if they are the only primary key.
            match self.0.columns.iter().filter(|c| c.auto_increment).count() {
                0 => true,
                1 => {
                    // In this case, the primary key **must** be the auto incrementing column
                    let Some(pk) = self.0.primary_key.as_deref().and_then(|pk| pk.as_record())
                    else {
                        todo!("Toasty should catch this earlier")
                    };

                    let [stmt::Expr::Reference(pk)] = &pk.fields[..] else {
                        todo!("Toasty should catch this earlier")
                    };

                    let pk = pk.as_expr_column_unwrap();

                    assert_eq!(0, pk.nesting);
                    assert!(
                        self.0.columns[pk.column].auto_increment,
                        "Toasty should catch this earlier"
                    );

                    false
                }
                _ => panic!("Toasty should catch this case earlier"),
            }
        } else {
            true
        };

        for (index, column) in self.0.columns.iter().enumerate() {
            fmt!(cx, f, "\n    " column);
            if index < self.0.columns.len() - 1 {
                fmt!(cx, f, ",");
            }
        }

        match &self.0.primary_key {
            Some(pk) if trailing_pk => fmt!(cx, f, ",\n    PRIMARY KEY " pk "\n"),
            _ => fmt!(cx, f, "\n"),
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

impl ToSql for &stmt::AddColumn {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.table(self.table);
        let table_name = Ident(&table.name);

        // Create new expression scope to serialize the statement
        let cx = cx.scope(table);

        fmt!(
            &cx, f, "ALTER TABLE " table_name " ADD COLUMN " self.column
        );
    }
}

impl ToSql for &stmt::AlterColumn {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.table(self.id.table);
        let table_name = Ident(&table.name);

        // Create new expression scope to serialize the statement
        let cx = cx.scope(table);

        let column_name = Ident(&self.column_def.name);

        match f.serializer.flavor {
            Flavor::Postgresql => match &self.changes {
                AlterColumnChanges {
                    new_name: Some(name),
                    new_ty: None,
                    new_not_null: None,
                    new_auto_increment: None,
                } => fmt!(&cx, f, "ALTER TABLE " table_name " RENAME COLUMN " column_name " TO " Ident(name.as_str())),
                AlterColumnChanges {
                    new_name: None,
                    new_ty: Some(ty),
                    new_not_null: None,
                    new_auto_increment: None,
                } => fmt!(&cx, f, "ALTER TABLE " table_name " ALTER COLUMN " column_name " TYPE " ty),
                AlterColumnChanges {
                    new_name: None,
                    new_ty: None,
                    new_not_null: Some(true),
                    new_auto_increment: None,
                } => fmt!(&cx, f, "ALTER TABLE " table_name " ALTER COLUMN " column_name " SET NOT NULL"),
                AlterColumnChanges {
                    new_name: None,
                    new_ty: None,
                    new_not_null: Some(false),
                    new_auto_increment: None,
                } => fmt!(&cx, f, "ALTER TABLE " table_name " ALTER COLUMN " column_name " DROP NOT NULL"),
                AlterColumnChanges {
                    new_name: None,
                    new_ty: None,
                    new_not_null: None,
                    new_auto_increment: Some(true),
                } => fmt!(&cx, f, "ALTER TABLE " table_name " ALTER COLUMN " column_name " ADD GENERATED BY DEFAULT AS IDENTITY"),
                AlterColumnChanges {
                    new_name: None,
                    new_ty: None,
                    new_not_null: None,
                    new_auto_increment: Some(false),
                } => fmt!(&cx, f, "ALTER TABLE " table_name " ALTER COLUMN " column_name " DROP IDENTITY"),
                _ => panic!("PostgreSQL does not support modifying multiple column properties in one ALTER TABLE statement")
            },
            Flavor::Mysql => {
                let new_column_def = ColumnDef {
                    name: self.changes.new_name.as_ref().unwrap_or(&self.column_def.name).clone(),
                    ty: self.changes.new_ty.as_ref().unwrap_or(&self.column_def.ty).clone(),
                    not_null: self.changes.new_not_null.unwrap_or(self.column_def.not_null),
                    auto_increment: self.changes.new_auto_increment.unwrap_or(self.column_def.auto_increment),
                };
                fmt!(&cx, f, "ALTER TABLE " table_name " CHANGE COLUMN " column_name " " new_column_def)
            },
            Flavor::Sqlite => match &self.changes {
                AlterColumnChanges {
                    new_name: Some(name),
                    new_ty: None,
                    new_not_null: None,
                    new_auto_increment: None,
                } => fmt!(&cx, f, "ALTER TABLE " table_name " RENAME COLUMN " column_name " TO " Ident(name.as_str())),
                _ => panic!("SQLite only supports renaming columns in ALTER TABLE statement")
            },
        }
    }
}

impl ToSql for &stmt::AlterTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match &self.action {
            stmt::AlterTableAction::RenameTo(new_name) => {
                fmt!(cx, f, "ALTER TABLE " self.name " RENAME TO " new_name);
            }
        }
    }
}

impl ToSql for &stmt::CopyTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let target_cols = Comma(self.columns.iter().map(|(target, _)| target));
        let source_cols = Comma(self.columns.iter().map(|(_, source)| source));
        fmt!(cx, f, "INSERT INTO " self.target " (" target_cols ") SELECT " source_cols " FROM " self.source);
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

        fmt!(&cx, f, "DELETE FROM " self.from self.filter);

        f.alias = prev;
    }
}

impl ToSql for &stmt::Filter {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        if let Some(expr) = &self.expr {
            fmt!(&cx, f, " WHERE " expr);
        }
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

impl ToSql for &stmt::DropColumn {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let table = f.serializer.table(self.table);
        let table_name = Ident(&table.name);
        let if_exists = if self.if_exists { "IF EXISTS " } else { "" };

        // Create new expression scope to serialize the statement
        let cx = cx.scope(table);

        fmt!(&cx, f, "ALTER TABLE " table_name " DROP COLUMN " if_exists self.name);
    }
}

impl ToSql for &stmt::DropIndex {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let if_exists = if self.if_exists { "IF EXISTS " } else { "" };
        fmt!(cx, f, "DROP INDEX " if_exists self.name);
    }
}

impl ToSql for &stmt::Pragma {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        if !f.serializer.is_sqlite() {
            panic!("\"PRAGMA\" statements only supported in SQLite");
        }
        match &self.value {
            Some(value) => fmt!(cx, f, "PRAGMA " self.name.as_str() " = " value.as_str()),
            None => fmt!(cx, f, "PRAGMA " self.name.as_str()),
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

        if returning.is_some() && f.serializer.is_mysql() {
            panic!("MySQL does not support the RETURNING clause with INSERT statements; returning={returning:#?}");
        }

        // Set insert context to provide column type information for VALUES
        let insert_ctx = match &self.target {
            stmt::InsertTarget::Table(table) => Some(crate::serializer::InsertContext {
                table_id: table.table,
                columns: table.columns.clone(),
            }),
            _ => None,
        };
        let prev_insert_context = f.insert_context.take();
        f.insert_context = insert_ctx;

        fmt!(
            &cx, f, "INSERT INTO " self.target " " self.source returning
        );

        f.insert_context = prev_insert_context;
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
        assert!(self.offset.is_none(), "TODO; {:#?}", self);

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
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                let fields = expr_record
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(i, expr)| match expr {
                        stmt::Expr::Reference(stmt::ExprReference::Column { .. }) => {
                            (expr, None, None)
                        }
                        _ => (expr, Some(" AS "), Some(ColumnAlias(i))),
                    });

                fmt!(cx, f, Comma(fields));
            }
            stmt::Returning::Expr(stmt::Expr::Value(stmt::Value::Record(value_record))) => {
                fmt!(cx, f, Comma(&value_record.fields));
            }
            _ => todo!("returning={self:#?}"),
        }
    }
}

impl ToSql for &stmt::Select {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let source_table = self.source.as_source_table();

        if source_table.from.is_empty() {
            fmt!(cx, f, "SELECT " self.returning)
        } else {
            fmt!(
                cx, f,
                "SELECT " self.returning " FROM " self.source self.filter
            );
        }
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
            stmt::Statement::AddColumn(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::AlterColumn(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::AlterTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::CopyTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::CreateIndex(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::CreateTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::DropColumn(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::DropIndex(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::DropTable(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Pragma(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Delete(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Insert(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Query(stmt) => stmt.to_sql(cx, f),
            stmt::Statement::Update(stmt) => stmt.to_sql(cx, f),
        }
    }
}

impl ToSql for &stmt::SourceTable {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        // Iterate over each TableWithJoins in the from clause
        for (i, table_with_joins) in self.from.iter().enumerate() {
            if i > 0 {
                fmt!(cx, f, ", ");
            }

            // Serialize the main table relation
            match &table_with_joins.relation {
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
            for join in &table_with_joins.joins {
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
}

impl ToSql for &stmt::TableRef {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        match self {
            stmt::TableRef::Table(table_id) => {
                let table_name = f.serializer.table_name(*table_id);
                fmt!(cx, f, table_name);
            }
            stmt::TableRef::Derived(table_derived) => fmt!(cx, f, table_derived),
            stmt::TableRef::Cte { nesting, index } => {
                assert!(f.depth >= *nesting, "nesting={nesting} depth={}", f.depth);

                let depth = f.depth - nesting;
                fmt!(cx, f, "cte_" depth "_" index);
            }
            stmt::TableRef::Arg(..) => panic!("unexpected TableRef argument; table_ref={self:#?}"),
        }
    }
}

impl ToSql for &stmt::TableDerived {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        debug_assert!(f.alias);

        f.depth += 1;
        fmt!(cx, f, "(" self.subquery ")");
        f.depth -= 1;
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
        let prev = mem::replace(&mut f.alias, false);

        let table = f.serializer.schema.table(self.target.as_table_unwrap());
        let assignments = (table, &self.assignments);

        // Create a new expression scope to serialize the statement
        let cx = cx.scope(self);

        let returning = self
            .returning
            .as_ref()
            .map(|returning| (" RETURNING ", returning));

        if returning.is_some() && f.serializer.is_mysql() {
            panic!("MySQL does not support the RETURNING clause with UPDATE statements; returning={returning:#?}");
        }

        assert!(
            self.condition.is_none(),
            "SQL does not support update conditions"
        );

        fmt!(&cx, f, "UPDATE " self.target " SET " assignments self.filter returning);

        f.alias = prev;
    }
}

impl ToSql for (&db::Table, &stmt::Assignments) {
    fn to_sql<P: Params>(self, cx: &ExprContext<'_>, f: &mut super::Formatter<'_, P>) {
        let assignments: Vec<_> = self.1.iter().collect();

        // TODO: ideally this could be used with TypeHintedField, but that is
        // actually pretty brittle as it is tied to an insert context instead of
        // being more generic. Being more generic would be ideal, but we really
        // should extract a more generic "scope walker" kind of thing from the
        // lowering logic.
        for (i, (projection, assignment)) in assignments.iter().enumerate() {
            if i > 0 {
                f.dst.push_str(", ");
            }

            let column = self.0.resolve(projection);
            let column_name = Ident(&column.name);

            // Serialize column name and equals sign
            column_name.to_sql(cx, f);
            f.dst.push_str(" = ");

            // For value expressions, provide type hint from the column
            if let stmt::Expr::Value(value) = &assignment.expr {
                let type_hint = Some(&column.ty);
                let placeholder = f.params.push(value, type_hint);
                placeholder.to_sql(cx, f);
            } else {
                assignment.expr.to_sql(cx, f);
            }
        }
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
        // MySQL requires ROW() keyword for table value constructors when used
        // in subqueries, but NOT in INSERT statements
        if f.serializer.is_mysql() && f.insert_context.is_none() {
            let rows = Comma(self.rows.iter().map(|row| ("ROW(", row, ")")));
            fmt!(cx, f, "VALUES " rows)
        } else {
            let rows = Comma(self.rows.iter());
            fmt!(cx, f, "VALUES " rows)
        }
    }
}
