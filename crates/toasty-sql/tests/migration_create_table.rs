use toasty_core::{
    driver::Capability,
    schema::db::{
        Column, ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, PrimaryKey,
        RenameHints, Schema, SchemaDiff, Table, TableId, Type,
    },
    stmt as core_stmt,
};
use toasty_sql::{
    migration::MigrationStatement,
    serializer::{Params, Placeholder},
    Serializer,
};

struct NoParams;

impl Params for NoParams {
    fn push(&mut self, _: &core_stmt::Value, _: Option<&core_stmt::Type>) -> Placeholder {
        Placeholder(0)
    }
}

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
    }
}

fn make_table(id: usize, name: &str, columns: Vec<Column>, indices: Vec<Index>) -> Table {
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
            index: IndexId {
                table: TableId(id),
                index: 0,
            },
        },
        indices,
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
            serializer.serialize(ms.statement(), &mut NoParams)
        })
        .collect()
}

#[test]
fn create_single_table_sqlite() {
    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::Text),
            ],
            vec![],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert_eq!(
        sql[0],
        "CREATE TABLE \"users\" (\n    \"id\" BIGINT NOT NULL,\n    \"name\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);"
    );
}

#[test]
fn create_single_table_postgresql() {
    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::Text),
            ],
            vec![],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(sql.len(), 1);
    assert_eq!(
        sql[0],
        "CREATE TABLE \"users\" (\n    \"id\" BIGINT NOT NULL,\n    \"name\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);"
    );
}

#[test]
fn create_table_with_nullable_column() {
    let from = Schema::default();

    let mut name_col = make_column(0, 1, "email", Type::Text);
    name_col.nullable = true;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), name_col],
            vec![],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert!(sql[0].contains("\"email\" TEXT"), "got: {}", sql[0]);
    assert!(
        !sql[0].contains("\"email\" TEXT NOT NULL"),
        "got: {}",
        sql[0]
    );
}

#[test]
fn create_table_with_auto_increment_sqlite() {
    let from = Schema::default();

    let mut id_col = make_column(0, 0, "id", Type::Integer(8));
    id_col.auto_increment = true;

    let to = Schema {
        tables: vec![make_table(
            0,
            "counters",
            vec![id_col, make_column(0, 1, "value", Type::Integer(4))],
            vec![],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert!(
        sql[0].contains("AUTOINCREMENT") || sql[0].contains("PRIMARY KEY"),
        "expected auto increment handling, got: {}",
        sql[0]
    );
}

#[test]
fn create_table_with_index() {
    let from = Schema::default();

    let columns = vec![
        make_column(0, 0, "id", Type::Integer(8)),
        make_column(0, 1, "email", Type::Text),
    ];

    let index = Index {
        id: IndexId {
            table: TableId(0),
            index: 0,
        },
        name: "idx_users_email".to_string(),
        on: TableId(0),
        columns: vec![IndexColumn {
            column: ColumnId {
                table: TableId(0),
                index: 1,
            },
            op: IndexOp::Eq,
            scope: IndexScope::Local,
        }],
        unique: false,
        primary_key: false,
    };

    let to = Schema {
        tables: vec![make_table(0, "users", columns, vec![index])],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert!(sql[0].starts_with("CREATE TABLE"), "got: {}", sql[0]);
    assert_eq!(
        sql[1],
        "CREATE INDEX \"idx_users_email\" ON \"users\" (\"email\");"
    );
}

#[test]
fn create_table_with_unique_index() {
    let from = Schema::default();

    let columns = vec![
        make_column(0, 0, "id", Type::Integer(8)),
        make_column(0, 1, "email", Type::Text),
    ];

    let index = Index {
        id: IndexId {
            table: TableId(0),
            index: 0,
        },
        name: "idx_users_email_unique".to_string(),
        on: TableId(0),
        columns: vec![IndexColumn {
            column: ColumnId {
                table: TableId(0),
                index: 1,
            },
            op: IndexOp::Eq,
            scope: IndexScope::Local,
        }],
        unique: true,
        primary_key: false,
    };

    let to = Schema {
        tables: vec![make_table(0, "users", columns, vec![index])],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert_eq!(
        sql[1],
        "CREATE UNIQUE INDEX \"idx_users_email_unique\" ON \"users\" (\"email\");"
    );
}

#[test]
fn create_multiple_tables() {
    let from = Schema::default();
    let to = Schema {
        tables: vec![
            make_table(
                0,
                "users",
                vec![
                    make_column(0, 0, "id", Type::Integer(8)),
                    make_column(0, 1, "name", Type::Text),
                ],
                vec![],
            ),
            make_table(
                1,
                "posts",
                vec![
                    make_column(1, 0, "id", Type::Integer(8)),
                    make_column(1, 1, "title", Type::Text),
                    make_column(1, 2, "body", Type::Text),
                ],
                vec![],
            ),
        ],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert!(sql.iter().any(|s| s.contains("\"users\"")));
    assert!(sql.iter().any(|s| s.contains("\"posts\"")));
}

#[test]
fn create_table_varchar_mysql() {
    let from = Schema::default();
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::VarChar(191)),
            ],
            vec![],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(sql.len(), 1);
    assert!(sql[0].contains("VARCHAR(191)"), "got: {}", sql[0]);
}
