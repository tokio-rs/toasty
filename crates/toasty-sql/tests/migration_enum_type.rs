use toasty_core::{
    driver::Capability,
    schema::db::{
        Column, ColumnId, EnumVariant, PrimaryKey, RenameHints, Schema, SchemaDiff, Table, TableId,
        Type, TypeEnum,
    },
    stmt as core_stmt,
};
use toasty_sql::{Serializer, migration::MigrationStatement};

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

fn make_enum_type(name: &str, variants: &[&str]) -> TypeEnum {
    TypeEnum {
        name: Some(name.to_string()),
        variants: variants
            .iter()
            .map(|v| EnumVariant {
                name: v.to_string(),
            })
            .collect(),
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

// --- PostgreSQL: CREATE TYPE before CREATE TABLE ---

#[test]
fn create_table_with_enum_postgresql() {
    let status_enum = make_enum_type("status", &["pending", "active", "done"]);

    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_enum)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    // Should emit CREATE TYPE before CREATE TABLE
    assert_eq!(sql.len(), 2);
    assert_eq!(
        sql[0],
        "CREATE TYPE \"status\" AS ENUM ('pending', 'active', 'done');"
    );
    assert!(sql[1].starts_with("CREATE TABLE \"tasks\""));
    assert!(sql[1].contains("\"status\" status NOT NULL"));
}

// --- PostgreSQL: ALTER TYPE ADD VALUE ---

#[test]
fn add_variant_postgresql() {
    let status_v1 = make_enum_type("status", &["pending", "active", "done"]);
    let status_v2 = make_enum_type("status", &["pending", "active", "done", "cancelled"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "ALTER TYPE \"status\" ADD VALUE 'cancelled';");
}

// --- PostgreSQL: multiple new variants ---

#[test]
fn add_multiple_variants_postgresql() {
    let status_v1 = make_enum_type("status", &["pending"]);
    let status_v2 = make_enum_type("status", &["pending", "active", "done"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(sql.len(), 2);
    assert_eq!(sql[0], "ALTER TYPE \"status\" ADD VALUE 'active';");
    assert_eq!(sql[1], "ALTER TYPE \"status\" ADD VALUE 'done';");
}

// --- Error: variant removal ---

#[test]
#[should_panic]
fn remove_variant_is_error() {
    let status_v1 = make_enum_type("status", &["pending", "active", "done"]);
    let status_v2 = make_enum_type("status", &["pending", "active"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
}

// --- Error: variant reorder ---

#[test]
#[should_panic]
fn reorder_variant_is_error() {
    let status_v1 = make_enum_type("status", &["pending", "active", "done"]);
    let status_v2 = make_enum_type("status", &["active", "pending", "done"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
}

// --- SQLite: CREATE TABLE with CHECK constraint ---

#[test]
fn create_table_with_enum_sqlite() {
    let status_enum = make_enum_type("status", &["pending", "active", "done"]);

    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_enum)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    // SQLite: no CREATE TYPE, just CREATE TABLE with CHECK
    assert_eq!(sql.len(), 1);
    assert!(sql[0].starts_with("CREATE TABLE \"tasks\""));
    assert!(sql[0].contains("CHECK (\"status\" IN ('pending', 'active', 'done'))"));
}

// --- MySQL: CREATE TABLE with inline ENUM ---

#[test]
fn create_table_with_enum_mysql() {
    let status_enum = make_enum_type("status", &["pending", "active", "done"]);

    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_enum)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    // MySQL: no CREATE TYPE, just CREATE TABLE with inline ENUM
    assert_eq!(sql.len(), 1);
    assert!(sql[0].starts_with("CREATE TABLE `tasks`"));
    assert!(sql[0].contains("ENUM('pending', 'active', 'done')"));
}

// --- No changes = no statements ---

#[test]
fn no_enum_changes_no_statements() {
    let status = make_enum_type("status", &["pending", "active"]);

    let schema = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&schema, &schema, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    assert!(stmts.is_empty());
}

// --- Shared enum type: CREATE TYPE emitted only once ---

#[test]
fn shared_enum_type_created_once() {
    let priority = make_enum_type("priority", &["low", "medium", "high"]);

    let from = Schema::default();
    let to = Schema {
        tables: vec![
            make_table(
                0,
                "tasks",
                vec![
                    make_column(0, 0, "id", Type::Integer(8)),
                    make_column(0, 1, "priority", Type::Enum(priority.clone())),
                ],
            ),
            make_table(
                1,
                "bugs",
                vec![
                    make_column(1, 0, "id", Type::Integer(8)),
                    make_column(1, 1, "priority", Type::Enum(priority)),
                ],
            ),
        ],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    // 1 CREATE TYPE + 2 CREATE TABLE = 3 statements
    assert_eq!(sql.len(), 3);
    // CREATE TYPE must come first
    assert_eq!(
        sql[0],
        "CREATE TYPE \"priority\" AS ENUM ('low', 'medium', 'high');"
    );
    // Both tables must be created (order may vary)
    let tables: Vec<&str> = sql[1..].iter().map(|s| s.as_str()).collect();
    assert!(
        tables
            .iter()
            .any(|s| s.starts_with("CREATE TABLE \"tasks\""))
    );
    assert!(
        tables
            .iter()
            .any(|s| s.starts_with("CREATE TABLE \"bugs\""))
    );
}

// --- MySQL: add variant emits ALTER TABLE MODIFY COLUMN ---

#[test]
fn add_variant_mysql() {
    let status_v1 = make_enum_type("status", &["pending", "active", "done"]);
    let status_v2 = make_enum_type("status", &["pending", "active", "done", "cancelled"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(sql.len(), 1);
    // MySQL uses CHANGE COLUMN to redefine the column with updated ENUM
    assert!(
        sql[0].contains("ENUM('pending', 'active', 'done', 'cancelled')"),
        "got: {}",
        sql[0]
    );
}

// --- SQLite: add variant triggers table recreation ---

#[test]
fn add_variant_sqlite() {
    let status_v1 = make_enum_type("status", &["pending", "active", "done"]);
    let status_v2 = make_enum_type("status", &["pending", "active", "done", "cancelled"]);

    let from = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v1)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "tasks",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "status", Type::Enum(status_v2)),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    // SQLite uses table recreation for column type changes
    // Should include: PRAGMA, CREATE TABLE (with updated CHECK), INSERT, DROP, ALTER TABLE RENAME, PRAGMA
    assert!(
        sql.len() >= 4,
        "expected table recreation sequence, got {} statements",
        sql.len()
    );
    // The new table should have the updated CHECK constraint
    let create = sql.iter().find(|s| s.starts_with("CREATE TABLE")).unwrap();
    assert!(
        create.contains("CHECK (\"status\" IN ('pending', 'active', 'done', 'cancelled'))"),
        "got: {create}"
    );
}
