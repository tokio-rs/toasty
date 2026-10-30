use toasty_core::{
    driver::Capability,
    schema::{
        db::{Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId, Type},
        diff,
    },
    stmt,
};
use toasty_sql::{Serializer, migration::MigrationStatement};

fn make_column(table: usize, index: usize, name: &str, comment: Option<&str>) -> Column {
    Column {
        id: ColumnId {
            table: TableId(table),
            index,
        },
        name: name.to_string(),
        comment: comment.map(str::to_string),
        ty: stmt::Type::String,
        storage_ty: Type::Text,
        nullable: false,
        primary_key: index == 0,
        auto_increment: false,
        versionable: false,
    }
}

fn make_table(comment: Option<&str>, columns: Vec<Column>) -> Table {
    Table {
        id: TableId(0),
        name: "users".to_string(),
        comment: comment.map(str::to_string),
        primary_key: PrimaryKey {
            columns: vec![ColumnId {
                table: TableId(0),
                index: 0,
            }],
            index: IndexId {
                table: TableId(0),
                index: 0,
            },
        },
        columns,
        indices: vec![],
    }
}

fn migration_sql(from: &Schema, to: &Schema, capability: &Capability) -> Vec<String> {
    let hints = diff::RenameHints::new();
    let diff = diff::Schema::from(from, to, &hints);
    MigrationStatement::from_diff(&diff, capability)
        .iter()
        .map(|statement| {
            let serializer = match capability.driver_name {
                "PostgreSQL" => Serializer::postgresql(statement.schema()),
                "MySQL" => Serializer::mysql(statement.schema()),
                "SQLite" => Serializer::sqlite(statement.schema()),
                name => panic!("unsupported test driver: {name}"),
            };
            serializer.serialize(statement.statement())
        })
        .collect()
}

#[test]
fn create_table_comments_postgresql() {
    let to = Schema {
        tables: vec![make_table(
            Some("User accounts"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", Some("Sign-in address")),
            ],
        )],
    };

    assert_eq!(
        migration_sql(&Schema::default(), &to, &Capability::POSTGRESQL),
        vec![
            "CREATE TABLE \"users\" (\n    \"id\" TEXT NOT NULL,\n    \"email\" TEXT NOT NULL,\n    PRIMARY KEY (\"id\")\n);",
            "COMMENT ON TABLE \"users\" IS 'User accounts';",
            "COMMENT ON COLUMN \"users\".\"email\" IS 'Sign-in address';",
        ]
    );
}

#[test]
fn create_table_comments_mysql() {
    let to = Schema {
        tables: vec![make_table(
            Some("User accounts"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", Some("Sign-in address")),
            ],
        )],
    };

    assert_eq!(
        migration_sql(&Schema::default(), &to, &Capability::MYSQL),
        vec![
            "CREATE TABLE `users` (\n    `id` TEXT NOT NULL,\n    `email` TEXT NOT NULL,\n    PRIMARY KEY (`id`)\n);",
            "ALTER TABLE `users` COMMENT = 'User accounts';",
            "ALTER TABLE `users` MODIFY COLUMN `email` TEXT NOT NULL COMMENT 'Sign-in address';",
        ]
    );
}

#[test]
fn comment_changes_postgresql() {
    let from = Schema {
        tables: vec![make_table(
            Some("Old table"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", Some("Old column")),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            Some("New table"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", None),
            ],
        )],
    };

    assert_eq!(
        migration_sql(&from, &to, &Capability::POSTGRESQL),
        vec![
            "COMMENT ON COLUMN \"users\".\"email\" IS NULL;",
            "COMMENT ON TABLE \"users\" IS 'New table';",
        ]
    );
}

#[test]
fn comment_changes_mysql() {
    let from = Schema {
        tables: vec![make_table(
            Some("Old table"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", Some("Old column")),
            ],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            Some("New table"),
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", None),
            ],
        )],
    };

    assert_eq!(
        migration_sql(&from, &to, &Capability::MYSQL),
        vec![
            "ALTER TABLE `users` MODIFY COLUMN `email` TEXT NOT NULL COMMENT '';",
            "ALTER TABLE `users` COMMENT = 'New table';",
        ]
    );
}

#[test]
fn unsupported_comments_are_ignored() {
    let from = Schema {
        tables: vec![make_table(
            Some("Old table"),
            vec![make_column(0, 0, "id", Some("Old column"))],
        )],
    };
    let to = Schema {
        tables: vec![make_table(
            Some("New table"),
            vec![make_column(0, 0, "id", Some("New column"))],
        )],
    };

    assert!(migration_sql(&from, &to, &Capability::SQLITE).is_empty());
}

#[test]
fn mysql_alter_column_preserves_comment() {
    let from = Schema {
        tables: vec![make_table(
            None,
            vec![
                make_column(0, 0, "id", None),
                make_column(0, 1, "email", Some("Sign-in address")),
            ],
        )],
    };
    let mut next_email = make_column(0, 1, "email", Some("Sign-in address"));
    next_email.storage_ty = Type::VarChar(255);
    let to = Schema {
        tables: vec![make_table(
            None,
            vec![make_column(0, 0, "id", None), next_email],
        )],
    };

    assert_eq!(
        migration_sql(&from, &to, &Capability::MYSQL),
        vec![
            "ALTER TABLE `users` CHANGE COLUMN `email` `email` VARCHAR(255) NOT NULL COMMENT 'Sign-in address';"
        ]
    );
}
