//! Exhaustive coverage of `db::Type` → SQL type rendering in
//! `toasty-sql/src/serializer/ty.rs`, across PostgreSQL, MySQL, and SQLite.
//!
//! Each non-`todo!()` `(Type variant × flavor)` combination is exercised
//! through a `CREATE TABLE` migration so the rendered column type surfaces in
//! the generated DDL. `todo!()` combinations are pinned with
//! `#[should_panic(expected = "...")]` so the test suite documents which
//! combinations intentionally panic today and tracks the exact panic message.

use expect_test::expect;
use toasty_core::{
    driver::Capability,
    schema::{
        db::{Column, ColumnId, EnumVariant, PrimaryKey, Schema, Table, TableId, Type, TypeEnum},
        diff,
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

    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, capability_for(flavor));
    serialize_migration(&stmts, flavor)
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BOOLEAN NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Boolean).join("\n"));
}

#[test]
fn boolean_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` BOOLEAN NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Boolean).join("\n"));
}

#[test]
fn boolean_sqlite() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BOOLEAN NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Boolean).join("\n"));
}

// ---------------------------------------------------------------------------
// Integer (signed)
// ---------------------------------------------------------------------------

#[test]
fn integer_2_postgresql_smallint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" SMALLINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Integer(2)).join("\n"));
}

#[test]
fn integer_2_mysql_smallint() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` SMALLINT NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Integer(2)).join("\n"));
}

#[test]
fn integer_2_sqlite_smallint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" SMALLINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Integer(2)).join("\n"));
}

#[test]
fn integer_4_postgresql_integer() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" INTEGER NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Integer(4)).join("\n"));
}

#[test]
fn integer_4_mysql_integer() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` INTEGER NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Integer(4)).join("\n"));
}

#[test]
fn integer_4_sqlite_integer() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" INTEGER NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Integer(4)).join("\n"));
}

#[test]
fn integer_8_postgresql_bigint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BIGINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Integer(8)).join("\n"));
}

#[test]
fn integer_8_mysql_bigint() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` BIGINT NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Integer(8)).join("\n"));
}

#[test]
fn integer_8_sqlite_bigint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BIGINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Integer(8)).join("\n"));
}

// ---------------------------------------------------------------------------
// UnsignedInteger
// ---------------------------------------------------------------------------

#[test]
fn unsigned_integer_1_mysql_tinyint_unsigned() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` TINYINT UNSIGNED NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::UnsignedInteger(1)).join("\n"));
}

#[test]
fn unsigned_integer_2_mysql_smallint_unsigned() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` SMALLINT UNSIGNED NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::UnsignedInteger(2)).join("\n"));
}

#[test]
fn unsigned_integer_4_mysql_int_unsigned() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` INT UNSIGNED NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::UnsignedInteger(4)).join("\n"));
}

#[test]
fn unsigned_integer_8_mysql_bigint_unsigned() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` BIGINT UNSIGNED NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::UnsignedInteger(8)).join("\n"));
}

#[test]
fn unsigned_integer_1_postgresql_promotes_to_smallint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" SMALLINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::UnsignedInteger(1)).join("\n"));
}

#[test]
fn unsigned_integer_2_postgresql_promotes_to_integer() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" INTEGER NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::UnsignedInteger(2)).join("\n"));
}

#[test]
fn unsigned_integer_4_postgresql_promotes_to_bigint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BIGINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::UnsignedInteger(4)).join("\n"));
}

#[test]
fn unsigned_integer_8_postgresql_promotes_to_bigint() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BIGINT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::UnsignedInteger(8)).join("\n"));
}

#[test]
fn unsigned_integer_4_sqlite_integer() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" INTEGER NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::UnsignedInteger(4)).join("\n"));
}

#[test]
fn unsigned_integer_8_sqlite_integer() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" INTEGER NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::UnsignedInteger(8)).join("\n"));
}

// ---------------------------------------------------------------------------
// Float
// ---------------------------------------------------------------------------

#[test]
fn float_4_postgresql_real() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" REAL NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Float(4)).join("\n"));
}

#[test]
fn float_8_postgresql_double_precision() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" DOUBLE PRECISION NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Float(8)).join("\n"));
}

#[test]
fn float_4_mysql_float() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` FLOAT NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Float(4)).join("\n"));
}

#[test]
fn float_8_mysql_double() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` DOUBLE NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Float(8)).join("\n"));
}

#[test]
fn float_4_sqlite_real() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" REAL NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Float(4)).join("\n"));
}

#[test]
fn float_8_sqlite_real() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" REAL NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Float(8)).join("\n"));
}

// ---------------------------------------------------------------------------
// Text & VarChar
// ---------------------------------------------------------------------------

#[test]
fn text_postgresql() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TEXT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Text).join("\n"));
}

#[test]
fn text_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` TEXT NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Text).join("\n"));
}

#[test]
fn text_sqlite() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TEXT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Text).join("\n"));
}

#[test]
fn varchar_postgresql() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" VARCHAR(255) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::VarChar(255)).join("\n"));
}

#[test]
fn varchar_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` VARCHAR(255) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::VarChar(255)).join("\n"));
}

#[test]
fn varchar_sqlite() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" VARCHAR(255) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::VarChar(255)).join("\n"));
}

// ---------------------------------------------------------------------------
// Uuid (PG only; MySQL / SQLite hit `todo!()`)
// ---------------------------------------------------------------------------

#[test]
fn uuid_postgresql() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" UUID NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Uuid).join("\n"));
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" NUMERIC NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Numeric(None)).join("\n"));
}

#[test]
fn numeric_with_precision_postgresql() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" NUMERIC(10, 2) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Numeric(Some((10, 2)))).join("\n"));
}

#[test]
fn numeric_with_precision_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` DECIMAL(10, 2) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Numeric(Some((10, 2)))).join("\n"));
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
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` BINARY(16) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Binary(16)).join("\n"));
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BYTEA NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Blob).join("\n"));
}

#[test]
fn blob_mysql_blob() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` BLOB NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Blob).join("\n"));
}

#[test]
fn blob_sqlite_blob() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BLOB NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Blob).join("\n"));
}

// ---------------------------------------------------------------------------
// Timestamp
// ---------------------------------------------------------------------------

#[test]
fn timestamp_postgresql_timestamptz() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TIMESTAMPTZ(6) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Timestamp(6)).join("\n"));
}

#[test]
fn timestamp_mysql_timestamp() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` TIMESTAMP(6) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Timestamp(6)).join("\n"));
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" DATE NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Date).join("\n"));
}

#[test]
fn date_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` DATE NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Date).join("\n"));
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TIME(3) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Time(3)).join("\n"));
}

#[test]
fn time_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` TIME(3) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Time(3)).join("\n"));
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
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TIMESTAMP(6) NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::DateTime(6)).join("\n"));
}

#[test]
fn datetime_mysql_datetime() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` DATETIME(6) NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::DateTime(6)).join("\n"));
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
    expect![[r#"
        CREATE TYPE "status" AS ENUM ('pending', 'active', 'done');
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" status NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Enum(status)).join("\n"));
}

#[test]
fn enum_mysql_inline() {
    let status = make_enum_type(None, &["pending", "active", "done"]);
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` ENUM('pending', 'active', 'done') NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Enum(status)).join("\n"));
}

#[test]
fn enum_sqlite_text_with_check() {
    let status = make_enum_type(None, &["pending", "active", "done"]);
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TEXT NOT NULL CHECK ("col" IN ('pending', 'active', 'done')),
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Enum(status)).join("\n"));
}

// ---------------------------------------------------------------------------
// List
// ---------------------------------------------------------------------------

#[test]
fn list_postgresql_array_suffix() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" BIGINT[] NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::List(Box::new(Type::Integer(8)))).join("\n"));
}

#[test]
fn list_postgresql_text_array() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TEXT[] NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::List(Box::new(Type::Text))).join("\n"));
}

#[test]
fn list_mysql_json() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` JSON NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::List(Box::new(Type::Integer(8)))).join("\n"));
}

#[test]
fn list_sqlite_text() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" TEXT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::List(Box::new(Type::Integer(8)))).join("\n"));
}

// ---------------------------------------------------------------------------
// Custom (pass-through)
// ---------------------------------------------------------------------------

#[test]
fn custom_pass_through_postgresql() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" CITEXT NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("postgresql", Type::Custom("CITEXT".to_string())).join("\n"));
}

#[test]
fn custom_pass_through_mysql() {
    expect![[r#"
        CREATE TABLE `t` (
            `id` BIGINT NOT NULL,
            `col` MEDIUMTEXT NOT NULL,
            PRIMARY KEY (`id`)
        );"#]]
    .assert_eq(&render_type("mysql", Type::Custom("MEDIUMTEXT".to_string())).join("\n"));
}

#[test]
fn custom_pass_through_sqlite() {
    expect![[r#"
        CREATE TABLE "t" (
            "id" BIGINT NOT NULL,
            "col" ANY NOT NULL,
            PRIMARY KEY ("id")
        );"#]]
    .assert_eq(&render_type("sqlite", Type::Custom("ANY".to_string())).join("\n"));
}
