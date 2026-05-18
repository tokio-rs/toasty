//! Verifies the serializer renders `INSERT` / `UPDATE` / `DELETE` shapes,
//! including the `RETURNING` clause where supported, and that MySQL (which has
//! no `RETURNING` support) panics as expected.
//!
//! Tests construct the AST directly so the serializer is exercised in
//! isolation — no lowering pipeline involved.

use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{
        self, Assignments, Delete, Expr, ExprColumn, Filter, Insert, InsertTable, InsertTarget,
        Returning, Source, Update, UpdateTarget, Values,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

#[derive(Clone, Copy)]
enum Flavor {
    Sqlite,
    Postgresql,
    Mysql,
}

/// Minimal `id INTEGER PRIMARY KEY, *cols` table. `cols` start at column index
/// 1 and are all `TEXT`. Stored as `id` for the PK column to keep predicates
/// easy to read in assertions.
fn make_table(id: usize, name: &str, cols: &[&str]) -> Table {
    let mut columns = vec![Column {
        id: ColumnId {
            table: TableId(id),
            index: 0,
        },
        name: "id".to_string(),
        ty: stmt::Type::I64,
        storage_ty: StorageType::Integer(8),
        nullable: false,
        primary_key: true,
        auto_increment: false,
        versionable: false,
    }];
    for (i, name) in cols.iter().enumerate() {
        columns.push(Column {
            id: ColumnId {
                table: TableId(id),
                index: i + 1,
            },
            name: (*name).to_string(),
            ty: stmt::Type::String,
            storage_ty: StorageType::Text,
            nullable: false,
            primary_key: false,
            auto_increment: false,
            versionable: false,
        });
    }
    Table {
        id: TableId(id),
        name: name.to_string(),
        columns,
        primary_key: PrimaryKey {
            columns: vec![ColumnId {
                table: TableId(id),
                index: 0,
            }],
            index: toasty_core::schema::db::IndexId {
                table: TableId(id),
                index: 0,
            },
        },
        indices: vec![],
    }
}

/// Reference to column `column` of the `table`th entry in `SourceTable::tables`.
fn col(table: usize, column: usize) -> Expr {
    Expr::column(ExprColumn {
        nesting: 0,
        table,
        column,
    })
}

fn render(flavor: Flavor, schema: &Schema, stmt: stmt::Statement) -> String {
    let sql_stmt = SqlStatement::from(stmt);
    match flavor {
        Flavor::Sqlite => Serializer::sqlite(schema).serialize(&sql_stmt),
        Flavor::Postgresql => Serializer::postgresql(schema).serialize(&sql_stmt),
        Flavor::Mysql => Serializer::mysql(schema).serialize(&sql_stmt),
    }
}

fn users_schema() -> Schema {
    Schema {
        tables: vec![make_table(0, "users", &["name"])],
    }
}

/// Build `INSERT INTO users (id, name) VALUES (1, 'a')`.
fn insert_basic(returning: Option<Returning>) -> stmt::Statement {
    let target = InsertTarget::Table(InsertTable {
        table: TableId(0),
        columns: vec![
            ColumnId {
                table: TableId(0),
                index: 0,
            },
            ColumnId {
                table: TableId(0),
                index: 1,
            },
        ],
    });

    let row = Expr::record([Expr::from(1i64), Expr::from("a")]);
    let source = stmt::Query::values(Values::new(vec![row]));

    Insert {
        target,
        source,
        returning,
    }
    .into()
}

/// Build `UPDATE users SET name = 'b' [WHERE id = 1]` with optional RETURNING.
fn update_stmt(with_where: bool, returning: Option<Returning>) -> stmt::Statement {
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("b"));

    let filter = if with_where {
        Filter::new(Expr::eq(col(0, 0), Expr::from(1i64)))
    } else {
        Filter::ALL
    };

    Update {
        target: UpdateTarget::Table(TableId(0)),
        assignments,
        filter,
        condition: stmt::Condition::default(),
        returning,
    }
    .into()
}

/// Build `DELETE FROM users [WHERE id = 1]` with optional RETURNING.
fn delete_stmt(with_where: bool, returning: Option<Returning>) -> stmt::Statement {
    let filter = if with_where {
        Filter::new(Expr::eq(col(0, 0), Expr::from(1i64)))
    } else {
        Filter::ALL
    };

    Delete {
        from: Source::from(TableId(0)),
        filter,
        returning,
        condition: stmt::Condition::default(),
    }
    .into()
}

/// Backtick (MySQL) or double-quote (PG/SQLite) ident quote char.
fn q(flavor: Flavor) -> char {
    match flavor {
        Flavor::Mysql => '`',
        _ => '"',
    }
}

// -----------------------------------------------------------------------------
// INSERT
// -----------------------------------------------------------------------------

#[test]
fn insert_basic_values() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let sql = render(flavor, &schema, insert_basic(None));
        let q = q(flavor);
        let expected = format!("INSERT INTO {q}users{q} ({q}id{q}, {q}name{q}) VALUES (1, 'a');");
        assert_eq!(sql, expected, "flavor mismatch for {expected:?}");
        assert!(
            !sql.contains("RETURNING"),
            "did not expect RETURNING in: {sql}"
        );
    }
}

#[test]
fn insert_with_returning() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    for flavor in [Flavor::Sqlite, Flavor::Postgresql] {
        let sql = render(flavor, &schema, insert_basic(returning.clone()));
        assert!(
            sql.contains("RETURNING "),
            "expected `RETURNING ` in: {sql}"
        );
        // The returning projection references the `id` column.
        let q = q(flavor);
        let ident = format!("{q}id{q}");
        assert!(
            sql.contains(&ident),
            "expected `{ident}` in returning: {sql}"
        );
    }
}

#[test]
#[should_panic(expected = "MySQL does not support the RETURNING clause with INSERT")]
fn insert_returning_panics_on_mysql() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    render(Flavor::Mysql, &schema, insert_basic(returning));
}

// -----------------------------------------------------------------------------
// UPDATE
// -----------------------------------------------------------------------------

#[test]
fn update_basic() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let sql = render(flavor, &schema, update_stmt(false, None));
        let q = q(flavor);
        // `UPDATE "users" AS tbl_0_0 SET "name" = 'b';`
        let expected = format!("UPDATE {q}users{q} AS tbl_0_0 SET {q}name{q} = 'b';");
        assert_eq!(sql, expected, "flavor mismatch for {expected:?}");
        assert!(!sql.contains(" WHERE "), "did not expect WHERE in: {sql}");
        assert!(
            !sql.contains("RETURNING"),
            "did not expect RETURNING in: {sql}"
        );
    }
}

#[test]
fn update_with_where() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let sql = render(flavor, &schema, update_stmt(true, None));
        assert!(sql.contains(" SET "), "expected ` SET ` in: {sql}");
        assert!(sql.contains(" WHERE "), "expected ` WHERE ` in: {sql}");
        // The Update serializer disables alias prefixing — `WHERE "id" = 1`,
        // not `WHERE tbl_0_0."id" = 1`.
        let q = q(flavor);
        let needle = format!("WHERE {q}id{q} = 1");
        assert!(sql.contains(&needle), "expected `{needle}` in: {sql}");
    }
}

#[test]
fn update_with_returning() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    for flavor in [Flavor::Sqlite, Flavor::Postgresql] {
        let sql = render(flavor, &schema, update_stmt(true, returning.clone()));
        assert!(
            sql.contains(" RETURNING "),
            "expected ` RETURNING ` in: {sql}"
        );
    }
}

#[test]
#[should_panic(expected = "MySQL does not support the RETURNING clause with UPDATE")]
fn update_returning_panics_on_mysql() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    render(Flavor::Mysql, &schema, update_stmt(true, returning));
}

// -----------------------------------------------------------------------------
// DELETE
// -----------------------------------------------------------------------------

#[test]
fn delete_basic() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let sql = render(flavor, &schema, delete_stmt(false, None));
        let q = q(flavor);
        let expected = format!("DELETE FROM {q}users{q} AS tbl_0_0;");
        assert_eq!(sql, expected, "flavor mismatch for {expected:?}");
        assert!(!sql.contains(" WHERE "), "did not expect WHERE in: {sql}");
    }
}

#[test]
fn delete_with_where() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let sql = render(flavor, &schema, delete_stmt(true, None));
        assert!(
            sql.starts_with("DELETE FROM "),
            "expected DELETE FROM prefix in: {sql}"
        );
        assert!(sql.contains(" WHERE "), "expected ` WHERE ` in: {sql}");
        // Delete enables alias prefixing — `WHERE tbl_0_0."id" = 1`.
        let q = q(flavor);
        let needle = format!("tbl_0_0.{q}id{q} = 1");
        assert!(sql.contains(&needle), "expected `{needle}` in: {sql}");
    }
}

/// MySQL has no `RETURNING` clause for any DML statement, so the serializer
/// must reject a `DELETE ... RETURNING` on this flavor.
///
/// The panic today comes from the unconditional `assert!(returning.is_none())`
/// in the Delete serializer, so the message is the assert text — once the
/// serializer learns to emit RETURNING for PG/SQLite (see #[ignore]'d tests
/// below), the MySQL branch should become an explicit panic mirroring INSERT
/// and UPDATE, and this `expected` substring should track it.
#[test]
#[should_panic(expected = "self.returning.is_none()")]
fn delete_with_returning_panics_on_mysql() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    render(Flavor::Mysql, &schema, delete_stmt(true, returning));
}

// PostgreSQL has supported `DELETE ... RETURNING` since 8.2; SQLite since
// 3.35. The serializer currently asserts `returning.is_none()` unconditionally
// in `impl ToSql for &stmt::Delete`, so the tests below would panic today —
// they are `#[ignore]`'d and pin the expected behavior once the serializer is
// taught to emit `RETURNING` for these flavors (mirroring the existing pattern
// for INSERT/UPDATE).

#[ignore = "DELETE+RETURNING is not yet implemented; serializer panics unconditionally"]
#[test]
fn delete_with_returning_postgresql() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    let sql = render(Flavor::Postgresql, &schema, delete_stmt(true, returning));
    assert!(
        sql.contains(" RETURNING "),
        "expected ` RETURNING ` in: {sql}"
    );
    assert!(
        sql.contains(r#"tbl_0_0."id""#),
        "expected projected column in: {sql}"
    );
}

#[ignore = "DELETE+RETURNING is not yet implemented; serializer panics unconditionally"]
#[test]
fn delete_with_returning_sqlite() {
    let schema = users_schema();
    let returning = Some(Returning::Project(Expr::record([col(0, 0)])));
    let sql = render(Flavor::Sqlite, &schema, delete_stmt(true, returning));
    assert!(
        sql.contains(" RETURNING "),
        "expected ` RETURNING ` in: {sql}"
    );
    assert!(
        sql.contains(r#"tbl_0_0."id""#),
        "expected projected column in: {sql}"
    );
}
