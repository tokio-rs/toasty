//! Exhaustive coverage of `db::Type` → SQL type rendering in
//! `toasty-sql/src/serializer/ty.rs`, across PostgreSQL, MySQL, and SQLite.
//!
//! Each non-`todo!()` `(Type variant × flavor)` combination is exercised
//! through a `CREATE TABLE` migration so the rendered column type surfaces in
//! the generated DDL. `todo!()` combinations are pinned with
//! `#[should_panic(expected = "...")]` so the test suite documents which
//! combinations intentionally panic today and tracks the exact panic message.

use toasty_core::{
    driver::Capability,
    schema::db::{
        Column, ColumnId, EnumVariant, PrimaryKey, RenameHints, Schema, SchemaDiff, Table, TableId,
        Type, TypeEnum,
    },
    stmt as core_stmt,
};
use toasty_sql::{Serializer, migration::MigrationStatement};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_column(table_id: usize, index: usize, name: &str, storage_ty: Type) -> Column {
    Column {
        id: ColumnId {
            table: TableId(table_id),
            index,
        },
        name: name.to_string(),
        ty: core_stmt::Type::String,
        storage_ty,
        nullable: false,
        primary_key: index == 0,
        auto_increment: false,
        versionable: false,
    }
}

fn make_table(id: usize, name: &str, columns: Vec<Column>) -> Table {
    let pk_columns: Vec<ColumnId> = columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.id)
        .collect();

    Table {
        id: TableId(id),
        name: name.to_string(),
        columns,
        primary_key: PrimaryKey {
            columns: pk_columns,
            index: toasty_core::schema::db::IndexId {
                table: TableId(id),
                index: 0,
            },
        },
        indices: vec![],
    }
}

fn serialize_migration(stmts: &[MigrationStatement<'_>], flavor: &str) -> Vec<String> {
    stmts
        .iter()
        .map(|ms| {
            let serializer = match flavor {
                "sqlite" => Serializer::sqlite(ms.schema()),
                "postgresql" => Serializer::postgresql(ms.schema()),
                "mysql" => Serializer::mysql(ms.schema()),
                _ => panic!("unknown flavor: {flavor}"),
            };
            serializer.serialize(ms.statement())
        })
        .collect()
}

fn capability_for(flavor: &str) -> &'static Capability {
    match flavor {
        "sqlite" => &Capability::SQLITE,
        "postgresql" => &Capability::POSTGRESQL,
        "mysql" => &Capability::MYSQL,
        _ => panic!("unknown flavor: {flavor}"),
    }
}

/// Build a one-column `CREATE TABLE` migration for the given storage type and
/// flavor, then return the rendered DDL strings.
fn render_type(flavor: &str, storage_ty: Type) -> Vec<String> {
    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "t",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "col", storage_ty),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, capability_for(flavor));
    serialize_migration(&stmts, flavor)
}

/// Convenience: assert that the `CREATE TABLE` for column `col` contains the
/// expected substring (the rendered storage type).
fn assert_col_type(sql: &[String], expected: &str) {
    assert!(
        sql.iter().any(|s| s.contains(expected)),
        "expected column type {expected:?} in: {sql:?}"
    );
}

fn make_enum_type(name: Option<&str>, variants: &[&str]) -> TypeEnum {
    TypeEnum {
        name: name.map(|s| s.to_string()),
        variants: variants
            .iter()
            .map(|v| EnumVariant {
                name: v.to_string(),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Boolean
// ---------------------------------------------------------------------------

#[test]
fn boolean_postgresql() {
    let sql = render_type("postgresql", Type::Boolean);
    assert_col_type(&sql, "\"col\" BOOLEAN");
}

#[test]
fn boolean_mysql() {
    let sql = render_type("mysql", Type::Boolean);
    assert_col_type(&sql, "`col` BOOLEAN");
}

#[test]
fn boolean_sqlite() {
    let sql = render_type("sqlite", Type::Boolean);
    assert_col_type(&sql, "\"col\" BOOLEAN");
}

// ---------------------------------------------------------------------------
// Integer (signed)
// ---------------------------------------------------------------------------

#[test]
fn integer_2_postgresql_smallint() {
    let sql = render_type("postgresql", Type::Integer(2));
    assert_col_type(&sql, "\"col\" SMALLINT");
}

#[test]
fn integer_2_mysql_smallint() {
    let sql = render_type("mysql", Type::Integer(2));
    assert_col_type(&sql, "`col` SMALLINT");
}

#[test]
fn integer_2_sqlite_smallint() {
    let sql = render_type("sqlite", Type::Integer(2));
    assert_col_type(&sql, "\"col\" SMALLINT");
}

#[test]
fn integer_4_postgresql_integer() {
    let sql = render_type("postgresql", Type::Integer(4));
    assert_col_type(&sql, "\"col\" INTEGER");
}

#[test]
fn integer_4_mysql_integer() {
    let sql = render_type("mysql", Type::Integer(4));
    assert_col_type(&sql, "`col` INTEGER");
}

#[test]
fn integer_4_sqlite_integer() {
    let sql = render_type("sqlite", Type::Integer(4));
    assert_col_type(&sql, "\"col\" INTEGER");
}

#[test]
fn integer_8_postgresql_bigint() {
    let sql = render_type("postgresql", Type::Integer(8));
    assert_col_type(&sql, "\"col\" BIGINT");
}

#[test]
fn integer_8_mysql_bigint() {
    let sql = render_type("mysql", Type::Integer(8));
    assert_col_type(&sql, "`col` BIGINT");
}

#[test]
fn integer_8_sqlite_bigint() {
    let sql = render_type("sqlite", Type::Integer(8));
    assert_col_type(&sql, "\"col\" BIGINT");
}

// ---------------------------------------------------------------------------
// UnsignedInteger
// ---------------------------------------------------------------------------

#[test]
fn unsigned_integer_1_mysql_tinyint_unsigned() {
    let sql = render_type("mysql", Type::UnsignedInteger(1));
    assert_col_type(&sql, "`col` TINYINT UNSIGNED");
}

#[test]
fn unsigned_integer_2_mysql_smallint_unsigned() {
    let sql = render_type("mysql", Type::UnsignedInteger(2));
    assert_col_type(&sql, "`col` SMALLINT UNSIGNED");
}

#[test]
fn unsigned_integer_4_mysql_int_unsigned() {
    let sql = render_type("mysql", Type::UnsignedInteger(4));
    assert_col_type(&sql, "`col` INT UNSIGNED");
}

#[test]
fn unsigned_integer_8_mysql_bigint_unsigned() {
    let sql = render_type("mysql", Type::UnsignedInteger(8));
    assert_col_type(&sql, "`col` BIGINT UNSIGNED");
}

#[test]
fn unsigned_integer_1_postgresql_promotes_to_smallint() {
    let sql = render_type("postgresql", Type::UnsignedInteger(1));
    assert_col_type(&sql, "\"col\" SMALLINT");
}

#[test]
fn unsigned_integer_2_postgresql_promotes_to_integer() {
    let sql = render_type("postgresql", Type::UnsignedInteger(2));
    assert_col_type(&sql, "\"col\" INTEGER");
}

#[test]
fn unsigned_integer_4_postgresql_promotes_to_bigint() {
    let sql = render_type("postgresql", Type::UnsignedInteger(4));
    assert_col_type(&sql, "\"col\" BIGINT");
}

#[test]
fn unsigned_integer_8_postgresql_promotes_to_bigint() {
    let sql = render_type("postgresql", Type::UnsignedInteger(8));
    assert_col_type(&sql, "\"col\" BIGINT");
}

#[test]
fn unsigned_integer_4_sqlite_integer() {
    let sql = render_type("sqlite", Type::UnsignedInteger(4));
    assert_col_type(&sql, "\"col\" INTEGER");
}

#[test]
fn unsigned_integer_8_sqlite_integer() {
    let sql = render_type("sqlite", Type::UnsignedInteger(8));
    assert_col_type(&sql, "\"col\" INTEGER");
}

// ---------------------------------------------------------------------------
// Float
// ---------------------------------------------------------------------------

#[test]
fn float_4_postgresql_real() {
    let sql = render_type("postgresql", Type::Float(4));
    assert_col_type(&sql, "\"col\" REAL");
}

#[test]
fn float_8_postgresql_double_precision() {
    let sql = render_type("postgresql", Type::Float(8));
    assert_col_type(&sql, "\"col\" DOUBLE PRECISION");
}

#[test]
fn float_4_mysql_float() {
    let sql = render_type("mysql", Type::Float(4));
    assert_col_type(&sql, "`col` FLOAT");
}

#[test]
fn float_8_mysql_double() {
    let sql = render_type("mysql", Type::Float(8));
    assert_col_type(&sql, "`col` DOUBLE");
}

#[test]
fn float_4_sqlite_real() {
    let sql = render_type("sqlite", Type::Float(4));
    assert_col_type(&sql, "\"col\" REAL");
}

#[test]
fn float_8_sqlite_real() {
    let sql = render_type("sqlite", Type::Float(8));
    assert_col_type(&sql, "\"col\" REAL");
}

// ---------------------------------------------------------------------------
// Text & VarChar
// ---------------------------------------------------------------------------

#[test]
fn text_postgresql() {
    let sql = render_type("postgresql", Type::Text);
    assert_col_type(&sql, "\"col\" TEXT");
}

#[test]
fn text_mysql() {
    let sql = render_type("mysql", Type::Text);
    assert_col_type(&sql, "`col` TEXT");
}

#[test]
fn text_sqlite() {
    let sql = render_type("sqlite", Type::Text);
    assert_col_type(&sql, "\"col\" TEXT");
}

#[test]
fn varchar_postgresql() {
    let sql = render_type("postgresql", Type::VarChar(255));
    assert_col_type(&sql, "\"col\" VARCHAR(255)");
}

#[test]
fn varchar_mysql() {
    let sql = render_type("mysql", Type::VarChar(255));
    assert_col_type(&sql, "`col` VARCHAR(255)");
}

#[test]
fn varchar_sqlite() {
    let sql = render_type("sqlite", Type::VarChar(255));
    assert_col_type(&sql, "\"col\" VARCHAR(255)");
}

// ---------------------------------------------------------------------------
// Uuid (PG only; MySQL / SQLite hit `todo!()`)
// ---------------------------------------------------------------------------

#[test]
fn uuid_postgresql() {
    let sql = render_type("postgresql", Type::Uuid);
    assert_col_type(&sql, "\"col\" UUID");
}

#[test]
#[should_panic(expected = "Unsupported type UUID")]
fn uuid_mysql_panics() {
    render_type("mysql", Type::Uuid);
}

#[test]
#[should_panic(expected = "Unsupported type UUID")]
fn uuid_sqlite_panics() {
    render_type("sqlite", Type::Uuid);
}

// ---------------------------------------------------------------------------
// Numeric
// ---------------------------------------------------------------------------

#[test]
fn numeric_unconstrained_postgresql() {
    let sql = render_type("postgresql", Type::Numeric(None));
    assert_col_type(&sql, "\"col\" NUMERIC");
}

#[test]
fn numeric_with_precision_postgresql() {
    let sql = render_type("postgresql", Type::Numeric(Some((10, 2))));
    assert_col_type(&sql, "\"col\" NUMERIC(10, 2)");
}

#[test]
fn numeric_with_precision_mysql() {
    let sql = render_type("mysql", Type::Numeric(Some((10, 2))));
    assert_col_type(&sql, "`col` DECIMAL(10, 2)");
}

#[test]
#[should_panic(expected = "MySQL does not support arbitrary-precision NUMERIC")]
fn numeric_unconstrained_mysql_panics() {
    render_type("mysql", Type::Numeric(None));
}

#[test]
#[should_panic(expected = "SQLite does not support NUMERIC type")]
fn numeric_unconstrained_sqlite_panics() {
    render_type("sqlite", Type::Numeric(None));
}

#[test]
#[should_panic(expected = "SQLite does not support NUMERIC type")]
fn numeric_with_precision_sqlite_panics() {
    render_type("sqlite", Type::Numeric(Some((10, 2))));
}

// ---------------------------------------------------------------------------
// Binary (MySQL only)
// ---------------------------------------------------------------------------

#[test]
fn binary_mysql() {
    let sql = render_type("mysql", Type::Binary(16));
    assert_col_type(&sql, "`col` BINARY(16)");
}

#[test]
#[should_panic(expected = "Unsupported fixed size binary type")]
fn binary_postgresql_panics() {
    render_type("postgresql", Type::Binary(16));
}

#[test]
#[should_panic(expected = "Unsupported fixed size binary type")]
fn binary_sqlite_panics() {
    render_type("sqlite", Type::Binary(16));
}

// ---------------------------------------------------------------------------
// Blob
// ---------------------------------------------------------------------------

#[test]
fn blob_postgresql_bytea() {
    let sql = render_type("postgresql", Type::Blob);
    assert_col_type(&sql, "\"col\" BYTEA");
}

#[test]
fn blob_mysql_blob() {
    let sql = render_type("mysql", Type::Blob);
    assert_col_type(&sql, "`col` BLOB");
}

#[test]
fn blob_sqlite_blob() {
    let sql = render_type("sqlite", Type::Blob);
    assert_col_type(&sql, "\"col\" BLOB");
}

// ---------------------------------------------------------------------------
// Timestamp
// ---------------------------------------------------------------------------

#[test]
fn timestamp_postgresql_timestamptz() {
    let sql = render_type("postgresql", Type::Timestamp(6));
    assert_col_type(&sql, "\"col\" TIMESTAMPTZ(6)");
}

#[test]
fn timestamp_mysql_timestamp() {
    let sql = render_type("mysql", Type::Timestamp(6));
    assert_col_type(&sql, "`col` TIMESTAMP(6)");
}

#[test]
#[should_panic(expected = "SQLite does not support Timestamp")]
fn timestamp_sqlite_panics() {
    render_type("sqlite", Type::Timestamp(6));
}

// ---------------------------------------------------------------------------
// Date
// ---------------------------------------------------------------------------

#[test]
fn date_postgresql() {
    let sql = render_type("postgresql", Type::Date);
    assert_col_type(&sql, "\"col\" DATE");
}

#[test]
fn date_mysql() {
    let sql = render_type("mysql", Type::Date);
    assert_col_type(&sql, "`col` DATE");
}

#[test]
#[should_panic(expected = "SQLite does not support Date")]
fn date_sqlite_panics() {
    render_type("sqlite", Type::Date);
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

#[test]
fn time_postgresql() {
    let sql = render_type("postgresql", Type::Time(3));
    assert_col_type(&sql, "\"col\" TIME(3)");
}

#[test]
fn time_mysql() {
    let sql = render_type("mysql", Type::Time(3));
    assert_col_type(&sql, "`col` TIME(3)");
}

#[test]
#[should_panic(expected = "SQLite does not support Time")]
fn time_sqlite_panics() {
    render_type("sqlite", Type::Time(3));
}

// ---------------------------------------------------------------------------
// DateTime
// ---------------------------------------------------------------------------

#[test]
fn datetime_postgresql_timestamp() {
    let sql = render_type("postgresql", Type::DateTime(6));
    assert_col_type(&sql, "\"col\" TIMESTAMP(6)");
}

#[test]
fn datetime_mysql_datetime() {
    let sql = render_type("mysql", Type::DateTime(6));
    assert_col_type(&sql, "`col` DATETIME(6)");
}

#[test]
#[should_panic(expected = "SQLite does not support DateTime")]
fn datetime_sqlite_panics() {
    render_type("sqlite", Type::DateTime(6));
}

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

#[test]
fn enum_postgresql_named_type() {
    let status = make_enum_type(Some("status"), &["pending", "active", "done"]);
    let sql = render_type("postgresql", Type::Enum(status));
    // Postgres references the named enum type; the actual `CREATE TYPE` is a
    // separate statement. The column DDL just names the type.
    assert_col_type(&sql, "\"col\" status NOT NULL");
}

#[test]
fn enum_mysql_inline() {
    let status = make_enum_type(None, &["pending", "active", "done"]);
    let sql = render_type("mysql", Type::Enum(status));
    assert_col_type(&sql, "`col` ENUM('pending', 'active', 'done')");
}

#[test]
fn enum_sqlite_text_with_check() {
    let status = make_enum_type(None, &["pending", "active", "done"]);
    let sql = render_type("sqlite", Type::Enum(status));
    // SQLite renders TEXT and adds a CHECK constraint in the column def.
    assert_col_type(&sql, "\"col\" TEXT");
    assert_col_type(&sql, "CHECK (\"col\" IN ('pending', 'active', 'done'))");
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

#[test]
fn list_postgresql_array_suffix() {
    let sql = render_type("postgresql", Type::List(Box::new(Type::Integer(8))));
    assert_col_type(&sql, "\"col\" BIGINT[]");
}

#[test]
fn list_postgresql_text_array() {
    let sql = render_type("postgresql", Type::List(Box::new(Type::Text)));
    assert_col_type(&sql, "\"col\" TEXT[]");
}

#[test]
fn list_mysql_json() {
    let sql = render_type("mysql", Type::List(Box::new(Type::Integer(8))));
    assert_col_type(&sql, "`col` JSON");
}

#[test]
fn list_sqlite_text() {
    let sql = render_type("sqlite", Type::List(Box::new(Type::Integer(8))));
    assert_col_type(&sql, "\"col\" TEXT");
}

// ---------------------------------------------------------------------------
// Custom (pass-through)
// ---------------------------------------------------------------------------

#[test]
fn custom_pass_through_postgresql() {
    let sql = render_type("postgresql", Type::Custom("CITEXT".to_string()));
    assert_col_type(&sql, "\"col\" CITEXT");
}

#[test]
fn custom_pass_through_mysql() {
    let sql = render_type("mysql", Type::Custom("MEDIUMTEXT".to_string()));
    assert_col_type(&sql, "`col` MEDIUMTEXT");
}

#[test]
fn custom_pass_through_sqlite() {
    let sql = render_type("sqlite", Type::Custom("ANY".to_string()));
    assert_col_type(&sql, "\"col\" ANY");
}
