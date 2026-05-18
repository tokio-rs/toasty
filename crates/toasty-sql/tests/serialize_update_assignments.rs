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

    let sql = render(Flavor::Postgresql, &schema, update_with(assignments));
    assert!(
        sql.contains(r#"SET "name" = 'x'"#),
        "expected `SET \"name\" = 'x'` in: {sql}"
    );
}

#[test]
fn set_assignment_mysql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("x"));

    let sql = render(Flavor::Mysql, &schema, update_with(assignments));
    assert!(
        sql.contains("SET `name` = 'x'"),
        "expected ``SET `name` = 'x'`` in: {sql}"
    );
}

#[test]
fn set_assignment_sqlite() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.set(1usize, Expr::from("x"));

    let sql = render(Flavor::Sqlite, &schema, update_with(assignments));
    assert!(
        sql.contains(r#"SET "name" = 'x'"#),
        "expected `SET \"name\" = 'x'` in: {sql}"
    );
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

    let sql = render(Flavor::Postgresql, &schema, update_with(assignments));
    // PostgreSQL uses `text[] || text[]` array concatenation.
    assert!(
        sql.contains(r#""tags" = "tags" || "#),
        "expected `\"tags\" = \"tags\" || ...` in: {sql}"
    );
}

#[test]
fn append_assignment_mysql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.append(2usize, Expr::list([Expr::from(7i64)]));

    let sql = render(Flavor::Mysql, &schema, update_with(assignments));
    // MySQL uses JSON_MERGE_PRESERVE for JSON-array append semantics.
    assert!(
        sql.contains("JSON_MERGE_PRESERVE(`tags`, "),
        "expected `JSON_MERGE_PRESERVE(`tags`, ...)` in: {sql}"
    );
}

#[test]
fn append_assignment_sqlite() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.append(2usize, Expr::list([Expr::from(7i64)]));

    let sql = render(Flavor::Sqlite, &schema, update_with(assignments));
    // SQLite uses a `json_group_array` / `json_each` UNION ALL subquery.
    assert!(
        sql.contains("SELECT json_group_array(value) FROM"),
        "expected SQLite json_group_array subquery in: {sql}"
    );
    assert!(
        sql.contains(r#"json_each("tags")"#),
        "expected `json_each(\"tags\")` in: {sql}"
    );
    assert!(
        sql.contains("UNION ALL SELECT value FROM json_each("),
        "expected UNION ALL json_each(rhs) in: {sql}"
    );
}

// -----------------------------------------------------------------------------
// Remove — PG `array_remove`; MySQL / SQLite panic in the serializer.
// -----------------------------------------------------------------------------

#[test]
fn remove_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));

    let sql = render(Flavor::Postgresql, &schema, update_with(assignments));
    assert!(
        sql.contains(r#"array_remove("tags", "#),
        "expected `array_remove(\"tags\", ...)` in: {sql}"
    );
}

#[test]
fn remove_assignment_mysql_panics() {
    // The MySQL branch of `serialize_remove` is a `panic!` — the lowering
    // is supposed to reject `vec_remove` on this backend before reaching
    // the serializer.
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));

    let result =
        std::panic::catch_unwind(|| render(Flavor::Mysql, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected MySQL `Remove` serialization to panic, got: {:?}",
        result.ok()
    );
}

#[test]
fn remove_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove(2usize, Expr::from(7i64));

    let result =
        std::panic::catch_unwind(|| render(Flavor::Sqlite, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected SQLite `Remove` serialization to panic, got: {:?}",
        result.ok()
    );
}

// -----------------------------------------------------------------------------
// Pop — PG 1-based array slice; MySQL / SQLite panic.
// -----------------------------------------------------------------------------

#[test]
fn pop_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);

    let sql = render(Flavor::Postgresql, &schema, update_with(assignments));
    // PG: `col[1:cardinality(col) - 1]` — drops the last element via slicing.
    assert!(
        sql.contains(r#""tags" = "tags"[1:cardinality("tags") - 1]"#),
        "expected PG array slice `\"tags\"[1:cardinality(\"tags\") - 1]` in: {sql}"
    );
}

#[test]
fn pop_assignment_mysql_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);

    let result =
        std::panic::catch_unwind(|| render(Flavor::Mysql, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected MySQL `Pop` serialization to panic, got: {:?}",
        result.ok()
    );
}

#[test]
fn pop_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.pop(2usize);

    let result =
        std::panic::catch_unwind(|| render(Flavor::Sqlite, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected SQLite `Pop` serialization to panic, got: {:?}",
        result.ok()
    );
}

// -----------------------------------------------------------------------------
// RemoveAt — PG prefix + suffix concat via array slices; MySQL / SQLite panic.
// -----------------------------------------------------------------------------

#[test]
fn remove_at_assignment_postgresql() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));

    let sql = render(Flavor::Postgresql, &schema, update_with(assignments));
    // `col[1:idx] || col[idx + 2:cardinality(col)]` — keep prefix, drop the
    // element at user-facing index `idx`, append suffix. PG arrays are
    // 1-based, so element at user index `i` lives at PG position `i + 1`.
    assert!(
        sql.contains(r#""tags"[1:3]"#),
        "expected prefix slice `\"tags\"[1:3]` in: {sql}"
    );
    assert!(
        sql.contains(r#""tags"[3 + 2:cardinality("tags")]"#),
        "expected suffix slice `\"tags\"[3 + 2:cardinality(\"tags\")]` in: {sql}"
    );
    assert!(
        sql.contains(r#""tags"[1:3] || "tags"[3 + 2:cardinality("tags")]"#),
        "expected concat of prefix and suffix slices in: {sql}"
    );
}

#[test]
fn remove_at_assignment_mysql_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));

    let result =
        std::panic::catch_unwind(|| render(Flavor::Mysql, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected MySQL `RemoveAt` serialization to panic, got: {:?}",
        result.ok()
    );
}

#[test]
fn remove_at_assignment_sqlite_panics() {
    let schema = users_schema();
    let mut assignments = Assignments::default();
    assignments.remove_at(2usize, Expr::from(3i64));

    let result =
        std::panic::catch_unwind(|| render(Flavor::Sqlite, &schema, update_with(assignments)));
    assert!(
        result.is_err(),
        "expected SQLite `RemoveAt` serialization to panic, got: {:?}",
        result.ok()
    );
}
