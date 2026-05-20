//! Verifies the serializer renders `JoinOp::Left` / `JoinOp::Inner` as
//! `LEFT JOIN` / `INNER JOIN` with the expected ON clauses, and that
//! multi-join chains (the shape produced by eager-loading a multi-step
//! `via` relation) serialize as the expected sequence of joins.
//!
//! The test constructs the AST directly so the serializer is exercised in
//! isolation — no lowering pipeline involved.

use expect_test::expect;
use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{
        self, Expr, ExprColumn, Filter, Join, JoinOp, Returning, Select, Source, SourceTable,
        SourceTableId, TableFactor, TableRef, TableWithJoins,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

/// Minimal `id INTEGER PRIMARY KEY, *cols` table. `cols` start at column index
/// 1 and are all `INTEGER`. Stored as `id` for the PK column to keep ON
/// expressions easy to read in assertions.
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

fn render_sqlite(schema: &Schema, stmt: stmt::Statement) -> String {
    let sql_stmt = SqlStatement::from(stmt);
    Serializer::sqlite(schema).serialize(&sql_stmt)
}

/// Build a `SELECT u.id FROM users u <join> posts p ON p.user_id = u.id`,
/// parameterized by the `JoinOp`.
fn select_with_join(constraint: JoinOp) -> stmt::Statement {
    let source = Source::Table(SourceTable {
        tables: vec![TableRef::Table(TableId(0)), TableRef::Table(TableId(1))],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![Join {
                table: SourceTableId(1),
                constraint,
            }],
        }],
    });

    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source,
        filter: Filter::ALL,
        distinct: false,
    };
    stmt::Statement::Query(stmt::Query::builder(select).build())
}

#[test]
fn left_join_renders_left_join_with_on_clause() {
    let schema = Schema {
        tables: vec![
            make_table(0, "users", &[]),
            make_table(1, "posts", &["user_id"]),
        ],
    };

    let on = Expr::eq(col(1, 1), col(0, 0));
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 LEFT JOIN "posts" AS tbl_0_1 ON tbl_0_1."user_id" = tbl_0_0."id";"#]].assert_eq(&render_sqlite(&schema, select_with_join(JoinOp::Left(on))));
}

#[test]
fn inner_join_renders_inner_join_with_on_clause() {
    let schema = Schema {
        tables: vec![
            make_table(0, "users", &[]),
            make_table(1, "posts", &["user_id"]),
        ],
    };

    let on = Expr::eq(col(1, 1), col(0, 0));
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 INNER JOIN "posts" AS tbl_0_1 ON tbl_0_1."user_id" = tbl_0_0."id";"#]].assert_eq(&render_sqlite(&schema, select_with_join(JoinOp::Inner(on))));
}

/// Multi-step join: `users LEFT JOIN posts ON ... LEFT JOIN comments ON ...`.
/// This is the shape an eager-loaded multi-step `via` relation would emit.
#[test]
fn multi_step_left_join_chain() {
    let schema = Schema {
        tables: vec![
            make_table(0, "users", &[]),
            make_table(1, "posts", &["user_id"]),
            make_table(2, "comments", &["post_id"]),
        ],
    };

    let source = Source::Table(SourceTable {
        tables: vec![
            TableRef::Table(TableId(0)),
            TableRef::Table(TableId(1)),
            TableRef::Table(TableId(2)),
        ],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![
                Join {
                    table: SourceTableId(1),
                    constraint: JoinOp::Left(Expr::eq(col(1, 1), col(0, 0))),
                },
                Join {
                    table: SourceTableId(2),
                    constraint: JoinOp::Left(Expr::eq(col(2, 1), col(1, 0))),
                },
            ],
        }],
    });

    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0), col(1, 0), col(2, 0)])),
        source,
        filter: Filter::ALL,
        distinct: false,
    };
    let stmt = stmt::Statement::Query(stmt::Query::builder(select).build());
    expect![[r#"SELECT tbl_0_0."id", tbl_0_1."id", tbl_0_2."id" FROM "users" AS tbl_0_0 LEFT JOIN "posts" AS tbl_0_1 ON tbl_0_1."user_id" = tbl_0_0."id" LEFT JOIN "comments" AS tbl_0_2 ON tbl_0_2."post_id" = tbl_0_1."id";"#]].assert_eq(&render_sqlite(&schema, stmt));
}

/// Mixed-kind chain: `users INNER JOIN posts ON ... LEFT JOIN comments ON ...`.
/// Confirms the per-join keyword selection is independent.
#[test]
fn mixed_inner_then_left_join() {
    let schema = Schema {
        tables: vec![
            make_table(0, "users", &[]),
            make_table(1, "posts", &["user_id"]),
            make_table(2, "comments", &["post_id"]),
        ],
    };

    let source = Source::Table(SourceTable {
        tables: vec![
            TableRef::Table(TableId(0)),
            TableRef::Table(TableId(1)),
            TableRef::Table(TableId(2)),
        ],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![
                Join {
                    table: SourceTableId(1),
                    constraint: JoinOp::Inner(Expr::eq(col(1, 1), col(0, 0))),
                },
                Join {
                    table: SourceTableId(2),
                    constraint: JoinOp::Left(Expr::eq(col(2, 1), col(1, 0))),
                },
            ],
        }],
    });

    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source,
        filter: Filter::ALL,
        distinct: false,
    };
    let stmt = stmt::Statement::Query(stmt::Query::builder(select).build());
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 INNER JOIN "posts" AS tbl_0_1 ON tbl_0_1."user_id" = tbl_0_0."id" LEFT JOIN "comments" AS tbl_0_2 ON tbl_0_2."post_id" = tbl_0_1."id";"#]].assert_eq(&render_sqlite(&schema, stmt));
}
