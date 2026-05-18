//! Verifies the low-level serialization helpers — `Value`, `Ident`, `Name`,
//! `ColumnAlias`, and `Placeholder` — render correctly, including their
//! per-flavor differences.
//!
//! Most cases wrap the value or expression in a bare `VALUES (...)` row so the
//! helper's output appears directly in the SQL. Identifier-quoting cases use a
//! minimal `SELECT col FROM users` against a real schema so column and table
//! identifiers actually get emitted. Each test constructs the AST directly so
//! the serializer is exercised in isolation — no lowering pipeline involved.

use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{
        self, Cte, Expr, ExprColumn, Filter, Returning, Select, Source, SourceTable, SourceTableId,
        TableFactor, TableRef, TableWithJoins, With,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

/// Minimal `id INTEGER PRIMARY KEY, *cols` table. Mirrors the helper in
/// `serialize_joins.rs`.
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
            ty: stmt::Type::I64,
            storage_ty: StorageType::Integer(8),
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

/// Render `expr` as the projection of a bare `VALUES` row. The result is
/// `VALUES (<expr>);` — useful for getting a value or expression rendered
/// in isolation, without a containing SELECT.
fn render_values_pg(expr: Expr) -> String {
    let values = stmt::Values::new(vec![Expr::record([expr])]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let stmt = SqlStatement::from(core_stmt);
    let schema = Schema::default();
    Serializer::postgresql(&schema).serialize(&stmt)
}

/// `SELECT id FROM users` against a schema with a single `users(id)` table.
fn select_id_from_users() -> stmt::Statement {
    let source = Source::Table(SourceTable {
        tables: vec![TableRef::Table(TableId(0))],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source,
        filter: Filter::ALL,
    };
    stmt::Statement::Query(stmt::Query::builder(select).build())
}

fn users_schema() -> Schema {
    Schema {
        tables: vec![make_table(0, "users", &[])],
    }
}

// -----------------------------------------------------------------------------
// Value literals (`serializer/value.rs`)
// -----------------------------------------------------------------------------

#[test]
fn value_null() {
    let sql = render_values_pg(Expr::Value(stmt::Value::Null));
    assert!(sql.contains("NULL"), "expected `NULL` in: {sql}");
}

#[test]
fn value_bool_true_and_false() {
    let t = render_values_pg(Expr::Value(stmt::Value::Bool(true)));
    assert!(t.contains("TRUE"), "expected `TRUE` in: {t}");

    let f = render_values_pg(Expr::Value(stmt::Value::Bool(false)));
    assert!(f.contains("FALSE"), "expected `FALSE` in: {f}");
}

#[test]
fn value_i64() {
    let sql = render_values_pg(Expr::Value(stmt::Value::I64(42)));
    assert!(sql.contains("42"), "expected `42` in: {sql}");
}

#[test]
fn value_string_simple() {
    let sql = render_values_pg(Expr::Value(stmt::Value::String("hello".into())));
    assert!(sql.contains("'hello'"), "expected `'hello'` in: {sql}");
}

#[test]
fn value_string_with_single_quote() {
    // Embedded single quotes are SQL-escaped by doubling them: `O'Brien`
    // serializes as `'O''Brien'`.
    let sql = render_values_pg(Expr::Value(stmt::Value::String("O'Brien".into())));
    assert!(
        sql.contains("'O''Brien'"),
        "expected escaped `'O''Brien'` in: {sql}"
    );
}

#[test]
fn value_list() {
    let list = stmt::Value::List(vec![stmt::Value::I64(1), stmt::Value::I64(2)]);
    let sql = render_values_pg(Expr::Value(list));
    assert!(sql.contains("(1, 2)"), "expected `(1, 2)` in: {sql}");
}

#[test]
fn value_record() {
    // `stmt::Value::Record` renders with `(...)` and comma-separated fields,
    // matching the `Record` arm in `serializer/value.rs`.
    let record = stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
        stmt::Value::I64(1),
        stmt::Value::I64(2),
    ]));
    let sql = render_values_pg(Expr::Value(record));
    assert!(sql.contains("(1, 2)"), "expected `(1, 2)` in: {sql}");
}

// -----------------------------------------------------------------------------
// Ident quoting (`serializer/ident.rs`)
// -----------------------------------------------------------------------------

#[test]
fn ident_quoting_postgresql_double_quote() {
    let schema = users_schema();
    let stmt = select_id_from_users();
    let sql = Serializer::postgresql(&schema).serialize(&SqlStatement::from(stmt));

    assert!(sql.contains(r#""users""#), "expected `\"users\"` in: {sql}");
    assert!(sql.contains(r#""id""#), "expected `\"id\"` in: {sql}");
}

#[test]
fn ident_quoting_sqlite_double_quote() {
    let schema = users_schema();
    let stmt = select_id_from_users();
    let sql = Serializer::sqlite(&schema).serialize(&SqlStatement::from(stmt));

    assert!(sql.contains(r#""users""#), "expected `\"users\"` in: {sql}");
    assert!(sql.contains(r#""id""#), "expected `\"id\"` in: {sql}");
}

#[test]
fn ident_quoting_mysql_backtick() {
    let schema = users_schema();
    let stmt = select_id_from_users();
    let sql = Serializer::mysql(&schema).serialize(&SqlStatement::from(stmt));

    assert!(
        sql.contains("`users`"),
        "expected backticked `users` in: {sql}"
    );
    assert!(sql.contains("`id`"), "expected backticked `id` in: {sql}");
    assert!(
        !sql.contains(r#""users""#),
        "did not expect double quotes in MySQL: {sql}"
    );
}

// -----------------------------------------------------------------------------
// Name qualified (`serializer/name.rs`)
// -----------------------------------------------------------------------------

#[test]
fn name_renders_qualified_period_separated() {
    // A multi-part `Name` should render as period-separated quoted segments.
    // `DropTable` is the simplest statement that surfaces `Name` directly in
    // its output.
    let schema = Schema::default();
    let drop = toasty_sql::stmt::DropTable {
        name: toasty_sql::stmt::Name(vec!["public".into(), "users".into()]),
        if_exists: false,
    };
    let stmt: SqlStatement = drop.into();
    let sql = Serializer::postgresql(&schema).serialize(&stmt);

    assert!(
        sql.contains(r#""public"."users""#),
        "expected `\"public\".\"users\"` in: {sql}"
    );
}

// -----------------------------------------------------------------------------
// ColumnAlias (`serializer/column.rs`)
// -----------------------------------------------------------------------------

/// `ColumnAlias` is emitted when projecting a column from a CTE or derived
/// subquery — the underlying schema column name isn't visible at that scope,
/// so the serializer uses a positional alias.
///
/// PostgreSQL / SQLite use 1-based `column<n+1>` (e.g. `column1`); MySQL uses
/// 0-based `column_<n>` (e.g. `column_0`). The format is set by
/// `serializer/column.rs::ColumnAlias::to_sql`.
fn select_from_cte() -> stmt::Statement {
    let cte_query = {
        let source = Source::Table(SourceTable {
            tables: vec![TableRef::Table(TableId(0))],
            from: vec![TableWithJoins {
                relation: TableFactor::Table(SourceTableId(0)),
                joins: vec![],
            }],
        });
        let select = Select {
            returning: Returning::Project(Expr::record([col(0, 0)])),
            source,
            filter: Filter::ALL,
        };
        stmt::Query::builder(select).build()
    };

    let outer_source = Source::Table(SourceTable {
        tables: vec![TableRef::Cte {
            nesting: 0,
            index: 0,
        }],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let outer_select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source: outer_source,
        filter: Filter::ALL,
    };
    stmt::Statement::Query(
        stmt::Query::builder(outer_select)
            .with(With {
                ctes: vec![Cte { query: cte_query }],
            })
            .build(),
    )
}

#[test]
fn column_alias_format_per_flavor() {
    let schema = users_schema();
    let stmt = select_from_cte();
    let sql_stmt = SqlStatement::from(stmt);

    let pg = Serializer::postgresql(&schema).serialize(&sql_stmt);
    assert!(
        pg.contains("column1"),
        "expected 1-based `column1` in PG: {pg}"
    );

    let sqlite = Serializer::sqlite(&schema).serialize(&sql_stmt);
    assert!(
        sqlite.contains("column1"),
        "expected 1-based `column1` in SQLite: {sqlite}"
    );

    let mysql = Serializer::mysql(&schema).serialize(&sql_stmt);
    assert!(
        mysql.contains("column_0"),
        "expected 0-based `column_0` in MySQL: {mysql}"
    );
}

// -----------------------------------------------------------------------------
// Placeholder (`serializer/params.rs`)
// -----------------------------------------------------------------------------

#[test]
fn placeholder_postgresql_dollar_n() {
    // `Expr::arg(0)` becomes `Placeholder(1)` (positions are 0-based; PG's
    // placeholder is 1-based).
    let record = Expr::record([Expr::arg(0), Expr::arg(1)]);
    let values = stmt::Values::new(vec![record]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let sql = Serializer::postgresql(&Schema::default()).serialize(&SqlStatement::from(core_stmt));

    assert!(sql.contains("$1"), "expected `$1` in PG: {sql}");
    assert!(sql.contains("$2"), "expected `$2` in PG: {sql}");
}

#[test]
fn placeholder_mysql_question_mark() {
    // MySQL uses bare `?` with no index. Order is preserved via the
    // `arg_positions` reordering channel returned by `serialize_with_arg_order`.
    let record = Expr::record([Expr::arg(0), Expr::arg(1)]);
    let values = stmt::Values::new(vec![record]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let sql = Serializer::mysql(&Schema::default()).serialize(&SqlStatement::from(core_stmt));

    assert!(sql.contains("?"), "expected `?` in MySQL: {sql}");
    assert!(
        !sql.contains("$1") && !sql.contains("?1"),
        "MySQL placeholder should not be indexed: {sql}"
    );
    // Two args -> two `?`.
    assert_eq!(
        sql.matches('?').count(),
        2,
        "expected two `?` placeholders in MySQL: {sql}"
    );
}

#[test]
fn placeholder_sqlite_question_n() {
    let record = Expr::record([Expr::arg(0), Expr::arg(1)]);
    let values = stmt::Values::new(vec![record]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let sql = Serializer::sqlite(&Schema::default()).serialize(&SqlStatement::from(core_stmt));

    assert!(sql.contains("?1"), "expected `?1` in SQLite: {sql}");
    assert!(sql.contains("?2"), "expected `?2` in SQLite: {sql}");
}

#[test]
fn placeholder_mysql_arg_order_reordering() {
    // `arg_positions` records the 0-based `Expr::Arg.position` of each `?`
    // placeholder in occurrence order. The caller (the MySQL driver) uses this
    // to reorder its params vec to match the SQL's placeholder order.
    //
    // Construct a record with `Expr::arg(1)` first and `Expr::arg(0)` second
    // to confirm the order tracks SQL occurrence, not numeric position.
    let record = Expr::record([Expr::arg(1), Expr::arg(0)]);
    let values = stmt::Values::new(vec![record]);
    let core_stmt: stmt::Statement = stmt::Query::values(values).into();
    let (_sql, arg_order) = Serializer::mysql(&Schema::default())
        .serialize_with_arg_order(&SqlStatement::from(core_stmt));

    assert_eq!(
        arg_order,
        vec![1, 0],
        "expected arg_order [1, 0] tracking SQL occurrence: got {arg_order:?}"
    );
}
