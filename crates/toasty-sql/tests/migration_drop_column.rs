use toasty_core::{
    driver::Capability,
    schema::{
        db::{
            Column, ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, PrimaryKey, Schema,
            Table, TableId, Type,
        },
        diff,
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
            index: IndexId {
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

#[test]
fn drop_column_sqlite() {
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::Text),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8))],
        )],
    };

    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "ALTER TABLE \"users\" DROP COLUMN \"name\";");
}

#[test]
fn drop_index_before_indexed_column() {
    let mut from_table = make_table(
        0,
        "users",
        vec![
            make_column(0, 0, "id", Type::Integer(8)),
            make_column(0, 1, "email", Type::Text),
        ],
    );
    from_table.indices.push(Index {
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
    });

    let from = Schema {
        tables: vec![from_table],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8))],
        )],
    };

    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(
        sql,
        [
            "DROP INDEX \"idx_users_email\";",
            "ALTER TABLE \"users\" DROP COLUMN \"email\";",
        ]
    );
}

#[test]
fn drop_column_postgresql() {
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "email", Type::Text),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8))],
        )],
    };

    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "ALTER TABLE \"users\" DROP COLUMN \"email\";");
}

#[test]
fn drop_multiple_columns() {
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::Text),
                make_column(0, 2, "email", Type::Text),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8))],
        )],
    };

    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert!(sql.iter().any(|s| s.contains("\"name\"")));
    assert!(sql.iter().any(|s| s.contains("\"email\"")));
    assert!(sql.iter().all(|s| s.contains("DROP COLUMN")));
}
