use toasty_core::{
    driver::Capability,
    schema::db::{
        Column, ColumnId, IndexId, PrimaryKey, RenameHints, Schema, SchemaDiff, Table, TableId,
        Type,
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
            serializer.serialize(ms.statement(), &mut NoParams)
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

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 1);
    assert_eq!(sql[0], "ALTER TABLE \"users\" DROP COLUMN \"name\";");
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

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
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

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql.len(), 2);
    assert!(sql.iter().any(|s| s.contains("\"name\"")));
    assert!(sql.iter().any(|s| s.contains("\"email\"")));
    assert!(sql.iter().all(|s| s.contains("DROP COLUMN")));
}
