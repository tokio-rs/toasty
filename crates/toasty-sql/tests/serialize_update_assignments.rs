//! Verifies the flavor-divergent serialization of `Assignments` in `UPDATE`
//! statements: `Set`, `Append`, `Remove`, `Pop`, and `RemoveAt`.
//!
//! Plain `Set` renders the same shape across all flavors (with backend ident
//! quoting). The collection operators (`Append`, `Remove`, `Pop`, `RemoveAt`)
//! target a `Vec<scalar>` column and render flavor-specific shapes. Currently
//! only PostgreSQL has implementations of `Remove` / `Pop` / `RemoveAt`; MySQL
//! and SQLite panic in the serializer because the lowering is supposed to
//! reject those backends earlier. The tests below capture that explicitly.
//!
//! Tests construct the AST directly so the serializer is exercised in
//! isolation — no lowering pipeline involved.

use expect_test::expect;
use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{self, Assignments, Expr, Filter, Update, UpdateTarget},
};
use toasty_sql::{Serializer, Statement as SqlStatement};

#[derive(Clone, Copy)]
enum Flavor {
    Sqlite,
    Postgresql,
    Mysql,
}

/// Build a `users` table with `id INTEGER PRIMARY KEY` plus the supplied
/// extra columns. Each extra column is either a scalar `TEXT` column or a
/// `Vec<scalar>` column (rendered as a `List(I64)` so the column has a list
/// type the collection operators can target).
fn make_table(id: usize, name: &str, cols: &[(&str, bool)]) -> Table {
    // (name, is_list)
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
    for (i, (col_name, is_list)) in cols.iter().enumerate() {
        let (ty, storage_ty) = if *is_list {
            (
                stmt::Type::List(Box::new(stmt::Type::I64)),
                StorageType::List(Box::new(StorageType::Integer(8))),
            )
        } else {
            (stmt::Type::String, StorageType::Text)
        };
        columns.push(Column {
            id: ColumnId {
                table: TableId(id),
                index: i + 1,
            },
            name: (*col_name).to_string(),
            ty,
            storage_ty,
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

fn render(flavor: Flavor, schema: &Schema, stmt: stmt::Statement) -> String {
    let sql_stmt = SqlStatement::from(stmt);
    match flavor {
        Flavor::Sqlite => Serializer::sqlite(schema).serialize(&sql_stmt),
        Flavor::Postgresql => Serializer::postgresql(schema).serialize(&sql_stmt),
        Flavor::Mysql => Serializer::mysql(schema).serialize(&sql_stmt),
    }
}

/// Schema with a `users` table holding `id`, scalar `name`, and list `tags`.
fn users_schema() -> Schema {
    Schema {
        tables: vec![make_table(0, "users", &[("name", false), ("tags", true)])],
    }
}

/// Build `UPDATE users SET <single assignment>` with no `WHERE`, no
/// `RETURNING`, and no `Condition`.
fn update_with(assignments: Assignments) -> stmt::Statement {
    Update {
        target: UpdateTarget::Table(TableId(0)),
        assignments,
        filter: Filter::ALL,
        condition: stmt::Condition::default(),
        returning: None,
    }
    .into()
}

// -----------------------------------------------------------------------------
// Set (plain assignment) — same shape across flavors, modulo ident quoting.
// -----------------------------------------------------------------------------

#[test]
fn set_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("x"));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "name" = 'x';"#]].assert_eq(&render(
        Flavor::Postgresql,
        &schema,
        update_with(assignments),
    ));
}

#[test]
fn set_assignment_mysql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("x"));

    expect!["UPDATE `users` AS tbl_0_0 SET `name` = 'x';"].assert_eq(&render(
        Flavor::Mysql,
        &schema,
        update_with(assignments),
    ));
}

#[test]
fn set_assignment_sqlite() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("x"));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "name" = 'x';"#]].assert_eq(&render(
        Flavor::Sqlite,
        &schema,
        update_with(assignments),
    ));
}

// -----------------------------------------------------------------------------
// Append — flavor-divergent: PG `||`, MySQL `JSON_MERGE_PRESERVE`, SQLite
// `json_each` subquery.
// -----------------------------------------------------------------------------

#[test]
fn append_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.append(2usize, Expr::list([Expr::from(7i64)]));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "tags" = "tags" || (7);"#]].assert_eq(&render(
        Flavor::Postgresql,
        &schema,
        update_with(assignments),
    ));
}

#[test]
fn append_assignment_mysql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.append(2usize, Expr::list([Expr::from(7i64)]));

    expect!["UPDATE `users` AS tbl_0_0 SET `tags` = JSON_MERGE_PRESERVE(`tags`, (7));"]
        .assert_eq(&render(Flavor::Mysql, &schema, update_with(assignments)));
}

#[test]
fn append_assignment_sqlite() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.append(2usize, Expr::list([Expr::from(7i64)]));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "tags" = (SELECT json_group_array(value) FROM (SELECT value FROM json_each("tags") UNION ALL SELECT value FROM json_each((7))));"#]].assert_eq(&render(Flavor::Sqlite, &schema, update_with(assignments)));
}

// -----------------------------------------------------------------------------
// Remove — PG `array_remove`; MySQL / SQLite panic in the serializer.
// -----------------------------------------------------------------------------

#[test]
fn remove_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "tags" = array_remove("tags", 7);"#]].assert_eq(
        &render(Flavor::Postgresql, &schema, update_with(assignments)),
    );
}

// The MySQL/SQLite branches of `serialize_remove` are `panic!` — the
// lowering is supposed to reject `vec_remove` on these backends before
// reaching the serializer.

#[test]
#[should_panic(expected = "stmt::remove on a Vec<scalar> field is not yet implemented")]
fn remove_assignment_mysql_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));
    render(Flavor::Mysql, &schema, update_with(assignments));
}

#[test]
#[should_panic(expected = "stmt::remove on a Vec<scalar> field is not yet implemented")]
fn remove_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));
    render(Flavor::Sqlite, &schema, update_with(assignments));
}

// -----------------------------------------------------------------------------
// Pop — PG 1-based array slice; MySQL / SQLite panic.
// -----------------------------------------------------------------------------

#[test]
fn pop_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "tags" = "tags"[1:cardinality("tags") - 1];"#]]
        .assert_eq(&render(
            Flavor::Postgresql,
            &schema,
            update_with(assignments),
        ));
}

#[test]
#[should_panic(expected = "stmt::pop on a Vec<scalar> field is not yet implemented")]
fn pop_assignment_mysql_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);
    render(Flavor::Mysql, &schema, update_with(assignments));
}

#[test]
#[should_panic(expected = "stmt::pop on a Vec<scalar> field is not yet implemented")]
fn pop_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);
    render(Flavor::Sqlite, &schema, update_with(assignments));
}

// -----------------------------------------------------------------------------
// RemoveAt — PG prefix + suffix concat via array slices; MySQL / SQLite panic.
// -----------------------------------------------------------------------------

#[test]
fn remove_at_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));

    expect![[r#"UPDATE "users" AS tbl_0_0 SET "tags" = "tags"[1:3] || "tags"[3 + 2:cardinality("tags")];"#]].assert_eq(&render(
        Flavor::Postgresql,
        &schema,
        update_with(assignments),
    ));
}

#[test]
#[should_panic(expected = "stmt::remove_at on a Vec<scalar> field is not yet implemented")]
fn remove_at_assignment_mysql_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));
    render(Flavor::Mysql, &schema, update_with(assignments));
}

#[test]
#[should_panic(expected = "stmt::remove_at on a Vec<scalar> field is not yet implemented")]
fn remove_at_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));
    render(Flavor::Sqlite, &schema, update_with(assignments));
}
