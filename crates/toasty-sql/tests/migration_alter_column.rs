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

// ---------------------------------------------------------------------------
// PostgreSQL: each property change is a separate statement
// ---------------------------------------------------------------------------

#[test]
fn alter_column_rename_postgresql() {
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

    let mut renamed = make_column(0, 1, "full_name", Type::Text);
    renamed.primary_key = false;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), renamed],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE \"users\" RENAME COLUMN \"name\" TO \"full_name\";",]
    );
}

#[test]
fn alter_column_rename_with_table_rename_postgresql() {
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

    let mut renamed = make_column(0, 1, "full_name", Type::Text);
    renamed.primary_key = false;

    let to = Schema {
        tables: vec![make_table(
            0,
            "accounts",
            vec![make_column(0, 0, "id", Type::Integer(8)), renamed],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec![
            "ALTER TABLE \"users\" RENAME TO \"accounts\";",
            "ALTER TABLE \"accounts\" RENAME COLUMN \"name\" TO \"full_name\";",
        ]
    );
}

#[test]
fn alter_column_set_not_null_postgresql() {
    let mut email = make_column(0, 1, "email", Type::Text);
    email.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), email],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "email", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE \"users\" ALTER COLUMN \"email\" SET NOT NULL;",]
    );
}

#[test]
fn alter_column_drop_not_null_postgresql() {
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

    let mut email = make_column(0, 1, "email", Type::Text);
    email.nullable = true;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), email],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE \"users\" ALTER COLUMN \"email\" DROP NOT NULL;",]
    );
}

#[test]
fn alter_column_change_type_postgresql() {
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Integer(4)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE \"users\" ALTER COLUMN \"value\" TYPE TEXT;",]
    );
}

#[test]
fn alter_column_multiple_changes_postgresql() {
    // Change type AND nullability → two separate statements
    let mut value = make_column(0, 1, "value", Type::Integer(4));
    value.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), value],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec![
            "ALTER TABLE \"users\" ALTER COLUMN \"value\" TYPE TEXT;",
            "ALTER TABLE \"users\" ALTER COLUMN \"value\" SET NOT NULL;",
        ]
    );
}

#[test]
fn alter_column_multiple_changes_with_table_rename_postgresql() {
    let mut value = make_column(0, 1, "value", Type::Integer(4));
    value.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), value],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "accounts",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::POSTGRESQL);
    let sql = serialize_migration(&stmts, "postgresql");

    assert_eq!(
        sql,
        vec![
            "ALTER TABLE \"users\" RENAME TO \"accounts\";",
            "ALTER TABLE \"accounts\" ALTER COLUMN \"value\" TYPE TEXT;",
            "ALTER TABLE \"accounts\" ALTER COLUMN \"value\" SET NOT NULL;",
        ]
    );
}

// ---------------------------------------------------------------------------
// MySQL: all property changes in a single CHANGE COLUMN statement
// ---------------------------------------------------------------------------

#[test]
fn alter_column_rename_mysql() {
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

    let mut renamed = make_column(0, 1, "full_name", Type::Text);
    renamed.primary_key = false;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), renamed],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE `users` CHANGE COLUMN `name` `full_name` TEXT NOT NULL;",]
    );
}

#[test]
fn alter_column_multiple_changes_mysql() {
    let mut value = make_column(0, 1, "value", Type::Integer(4));
    value.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), value],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(
        sql,
        vec!["ALTER TABLE `users` CHANGE COLUMN `value` `value` TEXT NOT NULL;",]
    );
}

#[test]
fn alter_column_multiple_changes_with_table_rename_mysql() {
    let mut value = make_column(0, 1, "value", Type::Integer(4));
    value.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), value],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "accounts",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::MYSQL);
    let sql = serialize_migration(&stmts, "mysql");

    assert_eq!(
        sql,
        vec![
            "ALTER TABLE `users` RENAME TO `accounts`;",
            "ALTER TABLE `accounts` CHANGE COLUMN `value` `value` TEXT NOT NULL;",
        ]
    );
}

// ---------------------------------------------------------------------------
// SQLite: rename-only works with ALTER TABLE RENAME COLUMN
// ---------------------------------------------------------------------------

#[test]
fn alter_column_rename_only_sqlite() {
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

    let mut renamed = make_column(0, 1, "full_name", Type::Text);
    renamed.primary_key = false;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), renamed],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(
        sql,
        vec!["ALTER TABLE \"users\" RENAME COLUMN \"name\" TO \"full_name\";",]
    );
}

// ---------------------------------------------------------------------------
// SQLite: non-rename changes require table recreation
// ---------------------------------------------------------------------------

#[test]
fn alter_column_change_nullability_sqlite() {
    // email: nullable → not null requires table recreation
    let mut email = make_column(0, 1, "email", Type::Text);
    email.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), email],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "email", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql, vec![
        "PRAGMA foreign_keys = OFF;",
        "CREATE TABLE \"_toasty_new_users\" (\n    \"id\" BIGINT NOT NULL,\n    \"email\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);",
        "INSERT INTO \"_toasty_new_users\" (\"id\", \"email\") SELECT \"id\", \"email\" FROM \"users\";",
        "DROP TABLE \"users\";",
        "ALTER TABLE \"_toasty_new_users\" RENAME TO \"users\";",
        "PRAGMA foreign_keys = ON;",
    ]);
}

#[test]
fn alter_column_change_type_sqlite() {
    // value: Integer(4) → Text requires table recreation
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Integer(4)),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "value", Type::Text),
            ],
        )],
    };

    let hints = RenameHints::new();
    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    assert_eq!(sql, vec![
        "PRAGMA foreign_keys = OFF;",
        "CREATE TABLE \"_toasty_new_users\" (\n    \"id\" BIGINT NOT NULL,\n    \"value\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);",
        "INSERT INTO \"_toasty_new_users\" (\"id\", \"value\") SELECT \"id\", \"value\" FROM \"users\";",
        "DROP TABLE \"users\";",
        "ALTER TABLE \"_toasty_new_users\" RENAME TO \"users\";",
        "PRAGMA foreign_keys = ON;",
    ]);
}

#[test]
fn alter_column_change_nullability_with_table_rename_sqlite() {
    // Table renamed users → accounts AND email: nullable → not null
    let mut email = make_column(0, 1, "email", Type::Text);
    email.nullable = true;

    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), email],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            0,
            "accounts",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "email", Type::Text),
            ],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    // The table rename happens first, then recreation uses the new name
    assert_eq!(sql, vec![
        "ALTER TABLE \"users\" RENAME TO \"accounts\";",
        "PRAGMA foreign_keys = OFF;",
        "CREATE TABLE \"_toasty_new_accounts\" (\n    \"id\" BIGINT NOT NULL,\n    \"email\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);",
        "INSERT INTO \"_toasty_new_accounts\" (\"id\", \"email\") SELECT \"id\", \"email\" FROM \"accounts\";",
        "DROP TABLE \"accounts\";",
        "ALTER TABLE \"_toasty_new_accounts\" RENAME TO \"accounts\";",
        "PRAGMA foreign_keys = ON;",
    ]);
}

#[test]
fn alter_column_rename_and_change_type_sqlite() {
    // Column renamed name → full_name AND type change Integer → Text
    // Both changes together require table recreation
    let from = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![
                make_column(0, 0, "id", Type::Integer(8)),
                make_column(0, 1, "name", Type::Integer(4)),
            ],
        )],
    };

    let mut full_name = make_column(0, 1, "full_name", Type::Text);
    full_name.primary_key = false;

    let to = Schema {
        tables: vec![make_table(
            0,
            "users",
            vec![make_column(0, 0, "id", Type::Integer(8)), full_name],
        )],
    };

    let mut hints = RenameHints::new();
    hints.add_column_hint(
        ColumnId {
            table: TableId(0),
            index: 1,
        },
        ColumnId {
            table: TableId(0),
            index: 1,
        },
    );

    let diff = SchemaDiff::from(&from, &to, &hints);
    let stmts = MigrationStatement::from_diff(&diff, &Capability::SQLITE);
    let sql = serialize_migration(&stmts, "sqlite");

    // Column rename + type change → table recreation with new column name
    assert_eq!(sql, vec![
        "PRAGMA foreign_keys = OFF;",
        "CREATE TABLE \"_toasty_new_users\" (\n    \"id\" BIGINT NOT NULL,\n    \"full_name\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);",
        "INSERT INTO \"_toasty_new_users\" (\"id\", \"full_name\") SELECT \"id\", \"name\" FROM \"users\";",
        "DROP TABLE \"users\";",
        "ALTER TABLE \"_toasty_new_users\" RENAME TO \"users\";",
        "PRAGMA foreign_keys = ON;",
    ]);
}
