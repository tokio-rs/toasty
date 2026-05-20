//! Verifies the serializer renders subquery-bearing shapes:
//!
//! * `VALUES` queries (bare and as `INSERT` sources, including MySQL's
//!   `ROW(...)` wrapper rule)
//! * `WITH` / CTE definitions and `cte_<depth>_<index>` references
//! * `TableDerived` (a subquery used as a `FROM` factor)
//! * `EXISTS` / `NOT EXISTS` predicates
//! * `IN (subquery)` predicates
//!
//! Each test constructs the AST directly so the serializer is exercised in
//! isolation — no lowering pipeline involved.

use expect_test::expect;
use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{
        self, Cte, Expr, ExprColumn, Filter, Insert, InsertTable, InsertTarget, Returning, Select,
        Source, SourceTable, SourceTableId, TableDerived, TableFactor, TableRef, TableWithJoins,
        Values, With,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

/// Minimal `id INTEGER PRIMARY KEY, *cols` table. `cols` start at column index
/// 1 and are all `INTEGER`. Mirrors the helper in `serialize_joins.rs`.
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
    Serializer::sqlite(schema).serialize(&SqlStatement::from(stmt))
}

fn render_postgresql(schema: &Schema, stmt: stmt::Statement) -> String {
    Serializer::postgresql(schema).serialize(&SqlStatement::from(stmt))
}

fn render_mysql(schema: &Schema, stmt: stmt::Statement) -> String {
    Serializer::mysql(schema).serialize(&SqlStatement::from(stmt))
}

fn users_schema() -> Schema {
    Schema {
        tables: vec![make_table(0, "users", &[])],
    }
}

/// `SELECT id FROM users` (column 0 of the only table in the source).
fn select_id_from_users() -> stmt::Query {
    let source = Source::Table(SourceTable {
        tables: vec![TableRef::Table(TableId(0))],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source,
        filter: Filter::ALL,
        distinct: false,
    };
    stmt::Query::builder(select).build()
}

// -----------------------------------------------------------------------------
// VALUES
// -----------------------------------------------------------------------------

#[test]
fn values_single_row_postgresql() {
    let schema = Schema::default();
    let values = Values::new(vec![Expr::record([Expr::from(1i64), Expr::from("a")])]);
    let stmt: stmt::Statement = stmt::Query::values(values).into();

    expect!["VALUES (1, 'a');"].assert_eq(&render_postgresql(&schema, stmt));
}

#[test]
fn values_multiple_rows_postgresql() {
    let schema = Schema::default();
    let values = Values::new(vec![
        Expr::record([Expr::from(1i64), Expr::from("a")]),
        Expr::record([Expr::from(2i64), Expr::from("b")]),
    ]);
    let stmt: stmt::Statement = stmt::Query::values(values).into();

    expect!["VALUES (1, 'a'), (2, 'b');"].assert_eq(&render_postgresql(&schema, stmt));
}

#[test]
fn values_uses_row_wrapper_on_mysql_outside_insert() {
    // Outside of INSERT, MySQL's table value constructor requires each row to
    // be wrapped in `ROW(...)`. The fields go directly inside `ROW(...)`,
    // not wrapped in an extra layer of parens — `ROW((1, 'a'))` is rejected
    // by MySQL as a single row-typed operand.
    let schema = Schema::default();
    let values = Values::new(vec![
        Expr::record([Expr::from(1i64), Expr::from("a")]),
        Expr::record([Expr::from(2i64), Expr::from("b")]),
    ]);
    let stmt: stmt::Statement = stmt::Query::values(values).into();

    expect!["VALUES ROW(1, 'a'), ROW(2, 'b');"].assert_eq(&render_mysql(&schema, stmt));
}

#[test]
fn values_inside_insert_no_row_wrapper_on_mysql() {
    // Inside INSERT, MySQL omits the `ROW(...)` wrapper.
    let schema = users_schema();
    let target = InsertTarget::Table(InsertTable {
        table: TableId(0),
        columns: vec![ColumnId {
            table: TableId(0),
            index: 0,
        }],
    });
    let values = Values::new(vec![
        Expr::record([Expr::from(1i64)]),
        Expr::record([Expr::from(2i64)]),
    ]);
    let stmt: stmt::Statement = Insert {
        target,
        source: stmt::Query::values(values),
        returning: None,
    }
    .into();

    expect!["INSERT INTO `users` (`id`) VALUES (1), (2);"].assert_eq(&render_mysql(&schema, stmt));
}

// -----------------------------------------------------------------------------
// WITH / CTE
// -----------------------------------------------------------------------------

#[test]
fn select_with_single_cte() {
    let schema = users_schema();

    let cte_query = select_id_from_users();

    // Outer query selects from the CTE: `SELECT col_0 FROM cte_0_0`. The CTE
    // is at index 0 in the outer query's `with` clause; the inner reference
    // has nesting=0 because, at serialize time, the CTE binding lives at the
    // outer query's depth which equals the body's depth.
    let outer_source = Source::Table(SourceTable {
        tables: vec![TableRef::Cte {
            nesting: 0,
            index: 0,
        }],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let outer_select = Select {
        // Reference column 0 of the CTE: the serializer resolves this as a
        // `ColumnAlias` (`col_0`) because the underlying table_ref is a CTE.
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source: outer_source,
        filter: Filter::ALL,
        distinct: false,
    };
    let stmt = stmt::Statement::Query(
        stmt::Query::builder(outer_select)
            .with(With {
                ctes: vec![Cte { query: cte_query }],
            })
            .build(),
    );

    expect![[r#"WITH cte_0_0 as (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0) SELECT tbl_0_0.column1 FROM cte_0_0 AS tbl_0_0;"#]].assert_eq(&render_sqlite(&schema, stmt));
}

#[test]
fn select_with_multiple_ctes() {
    let schema = users_schema();

    let outer_source = Source::Table(SourceTable {
        tables: vec![TableRef::Cte {
            nesting: 0,
            index: 0,
        }],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let outer_select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source: outer_source,
        filter: Filter::ALL,
        distinct: false,
    };
    let stmt = stmt::Statement::Query(
        stmt::Query::builder(outer_select)
            .with(With {
                ctes: vec![
                    Cte {
                        query: select_id_from_users(),
                    },
                    Cte {
                        query: select_id_from_users(),
                    },
                ],
            })
            .build(),
    );

    expect![[r#"WITH cte_0_0 as (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0), cte_0_1 as (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0) SELECT tbl_0_0.column1 FROM cte_0_0 AS tbl_0_0;"#]].assert_eq(&render_sqlite(&schema, stmt));
}

// -----------------------------------------------------------------------------
// Derived table in FROM
// -----------------------------------------------------------------------------

#[test]
fn select_from_derived_subquery() {
    let schema = users_schema();

    let inner = select_id_from_users();

    let outer_source = Source::Table(SourceTable {
        tables: vec![TableRef::Derived(TableDerived {
            subquery: Box::new(inner),
        })],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let outer_select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source: outer_source,
        filter: Filter::ALL,
        distinct: false,
    };
    let stmt = stmt::Statement::Query(stmt::Query::builder(outer_select).build());

    expect![[
        r#"SELECT tbl_0_0.column1 FROM (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0) AS tbl_0_0;"#
    ]]
    .assert_eq(&render_sqlite(&schema, stmt));
}

// -----------------------------------------------------------------------------
// EXISTS / NOT EXISTS
// -----------------------------------------------------------------------------

fn select_users_with_filter(filter: Expr) -> stmt::Statement {
    let source = Source::Table(SourceTable {
        tables: vec![TableRef::Table(TableId(0))],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    });
    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source,
        filter: Filter::new(filter),
        distinct: false,
    };
    stmt::Statement::Query(stmt::Query::builder(select).build())
}

#[test]
fn expr_exists_subquery() {
    let schema = users_schema();
    let exists = Expr::exists(select_id_from_users());

    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE EXISTS (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0);"#]].assert_eq(&render_postgresql(
        &schema,
        select_users_with_filter(exists.clone()),
    ));
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE EXISTS (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0);"#]].assert_eq(&render_sqlite(&schema, select_users_with_filter(exists)));
}

#[test]
fn expr_not_exists_subquery() {
    let schema = users_schema();
    // `Expr::not_exists` wraps `Exists` in `Not`, which the serializer renders
    // as `NOT (EXISTS (...))`.
    let not_exists = Expr::not_exists(select_id_from_users());

    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE NOT (EXISTS (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0));"#]].assert_eq(&render_postgresql(
        &schema,
        select_users_with_filter(not_exists.clone()),
    ));
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE NOT (EXISTS (SELECT tbl_1_0."id" FROM "users" AS tbl_1_0));"#]].assert_eq(&render_sqlite(
        &schema,
        select_users_with_filter(not_exists),
    ));
}

// -----------------------------------------------------------------------------
// IN (subquery)
// -----------------------------------------------------------------------------

#[test]
fn expr_in_subquery() {
    let schema = users_schema();
    let in_sub = Expr::in_subquery(col(0, 0), select_id_from_users());

    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE tbl_0_0."id" IN (SELECT tbl_0_0."id" FROM "users" AS tbl_0_0);"#]].assert_eq(&render_postgresql(
        &schema,
        select_users_with_filter(in_sub.clone()),
    ));
    expect![[r#"SELECT tbl_0_0."id" FROM "users" AS tbl_0_0 WHERE tbl_0_0."id" IN (SELECT tbl_0_0."id" FROM "users" AS tbl_0_0);"#]].assert_eq(&render_sqlite(&schema, select_users_with_filter(in_sub)));
}
