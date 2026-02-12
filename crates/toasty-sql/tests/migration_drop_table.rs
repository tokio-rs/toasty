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
fn drop_single_table_sqlite() {
    let from = Schema {
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
    let to = Schema::default();

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "DROP TABLE \"users\";");
}

#[test]
fn drop_single_table_postgresql() {
    let from = Schema {
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
    let to = Schema::default();

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "DROP TABLE \"users\";");
}

#[test]
fn drop_multiple_tables() {
    let from = Schema {
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
                ],
                vec![],
            ),
        ],
    };
    let to = Schema::default();

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert!(sql.iter().any(|s| s == "DROP TABLE \"users\";"));
    assert!(sql.iter().any(|s| s == "DROP TABLE \"posts\";"));
}

#[test]
fn drop_one_table_keep_another() {
    let users = make_table(
        0,
        "users",
        vec![
            make_column(0, 0, "id", Type::Integer(8)),
            make_column(0, 1, "name", Type::Text),
        ],
        vec![],
    );
    let posts = make_table(
        1,
        "posts",
        vec![
            make_column(1, 0, "id", Type::Integer(8)),
            make_column(1, 1, "title", Type::Text),
        ],
        vec![],
    );

    let from = Schema {
        tables: vec![users.clone(), posts],
    };
    let to = Schema {
        tables: vec![users],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "DROP TABLE \"posts\";");
}

#[test]
fn drop_table_with_index() {
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

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "email", Type::Text),
            ],
            vec![index],
        )],
    };
    let to = Schema::default();

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    // Dropping a table should just drop the table; indices are dropped implicitly.
    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "DROP TABLE \"users\";");
}

#[test]
fn drop_table_mysql() {
    let from = Schema {
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
    let to = Schema::default();

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "DROP TABLE `users`;");
}
