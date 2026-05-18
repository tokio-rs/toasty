//! Verifies the serializer renders `SELECT` shape — WHERE, ORDER BY,
//! LIMIT/OFFSET, and `FOR UPDATE` / `FOR SHARE` row-level locks.
//!
//! Joins and ANY/ALL operators have their own files; this file focuses on the
//! clauses around the `SELECT` body. Each test constructs the AST directly so
//! the serializer is exercised in isolation — no lowering pipeline involved.

use toasty_core::{
    schema::db::{Column, ColumnId, PrimaryKey, Schema, Table, TableId, Type as StorageType},
    stmt::{
        self, Direction, Expr, ExprColumn, Filter, Limit, LimitOffset, Lock, OrderBy, OrderByExpr,
        Returning, Select, Source, SourceTable, SourceTableId, TableFactor, TableRef,
        TableWithJoins,
    },
};
use toasty_sql::{Serializer, Statement as SqlStatement};

/// Minimal `id INTEGER PRIMARY KEY, *cols` table. `cols` start at column index
/// 1 and are all `INTEGER`.
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

/// SQL flavor selector for the `render` helper.
#[derive(Clone, Copy)]
enum Flavor {
    Sqlite,
    Postgresql,
    Mysql,
}

impl Flavor {
    fn name(self) -> &'static str {
        match self {
            Flavor::Sqlite => "sqlite",
            Flavor::Postgresql => "postgresql",
            Flavor::Mysql => "mysql",
        }
    }
}

fn render(flavor: Flavor, schema: &Schema, stmt: stmt::Statement) -> String {
    let sql_stmt = SqlStatement::from(stmt);
    let serializer = match flavor {
        Flavor::Sqlite => Serializer::sqlite(schema),
        Flavor::Postgresql => Serializer::postgresql(schema),
        Flavor::Mysql => Serializer::mysql(schema),
    };
    serializer.serialize(&sql_stmt)
}

/// Schema with a single `users(id, name)` table.
fn users_schema() -> Schema {
    Schema {
        tables: vec![make_table(0, "users", &["name"])],
    }
}

/// Single-table `users` source.
fn users_source() -> Source {
    Source::Table(SourceTable {
        tables: vec![TableRef::Table(TableId(0))],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    })
}

/// `SELECT id FROM users` with the given filter, ordering, limit, and locks.
fn make_query(
    filter: Filter,
    order_by: Option<OrderBy>,
    limit: Option<Limit>,
    locks: Vec<Lock>,
) -> stmt::Statement {
    let select = Select {
        returning: Returning::Project(Expr::record([col(0, 0)])),
        source: users_source(),
        filter,
    };
    let mut query = stmt::Query::builder(select).build();
    query.order_by = order_by;
    query.limit = limit;
    query.locks = locks;
    stmt::Statement::Query(query)
}

/// Returns the identifier quote character used by `flavor` — `"` for
/// SQLite/PostgreSQL, `` ` `` for MySQL.
fn q(flavor: Flavor) -> char {
    match flavor {
        Flavor::Sqlite | Flavor::Postgresql => '"',
        Flavor::Mysql => '`',
    }
}

#[test]
fn select_basic() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let stmt = make_query(Filter::ALL, None, None, vec![]);
        let sql = render(flavor, &schema, stmt);
        let q = q(flavor);
        let expected = format!("SELECT tbl_0_0.{q}id{q} FROM {q}users{q} AS tbl_0_0");
        assert!(
            sql.contains(&expected),
            "[{}] expected `{expected}` in: {sql}",
            flavor.name()
        );
        assert!(
            !sql.contains("WHERE"),
            "[{}] did not expect WHERE in: {sql}",
            flavor.name()
        );
    }
}

#[test]
fn select_with_where() {
    let schema = users_schema();
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let filter = Filter::from(Expr::eq(col(0, 0), Expr::from(1i64)));
        let stmt = make_query(filter, None, None, vec![]);
        let sql = render(flavor, &schema, stmt);
        let q = q(flavor);
        let expected = format!("WHERE tbl_0_0.{q}id{q} = 1");
        assert!(
            sql.contains(&expected),
            "[{}] expected `{expected}` in: {sql}",
            flavor.name()
        );
    }
}

#[test]
fn select_with_and_filter() {
    let schema = users_schema();
    let filter = Filter::from(Expr::and(
        Expr::eq(col(0, 0), Expr::from(1i64)),
        Expr::eq(col(0, 1), Expr::from("foo")),
    ));
    let stmt = make_query(filter, None, None, vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains(r#"WHERE tbl_0_0."id" = 1 AND tbl_0_0."name" = 'foo'"#),
        "expected AND filter in: {sql}"
    );
}

#[test]
fn select_with_or_filter() {
    let schema = users_schema();
    let filter = Filter::from(Expr::or(
        Expr::eq(col(0, 0), Expr::from(1i64)),
        Expr::eq(col(0, 0), Expr::from(2i64)),
    ));
    let stmt = make_query(filter, None, None, vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains(r#"WHERE tbl_0_0."id" = 1 OR tbl_0_0."id" = 2"#),
        "expected OR filter in: {sql}"
    );
}

#[test]
fn select_with_order_by_asc() {
    let schema = users_schema();
    let order_by = OrderBy {
        exprs: vec![OrderByExpr {
            expr: col(0, 0),
            order: Some(Direction::Asc),
        }],
    };
    let stmt = make_query(Filter::ALL, Some(order_by), None, vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains(r#"ORDER BY tbl_0_0."id" ASC"#),
        "expected `ORDER BY tbl_0_0.\"id\" ASC` in: {sql}"
    );
}

#[test]
fn select_with_order_by_desc() {
    let schema = users_schema();
    let order_by = OrderBy {
        exprs: vec![OrderByExpr {
            expr: col(0, 0),
            order: Some(Direction::Desc),
        }],
    };
    let stmt = make_query(Filter::ALL, Some(order_by), None, vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains(r#"ORDER BY tbl_0_0."id" DESC"#),
        "expected `ORDER BY tbl_0_0.\"id\" DESC` in: {sql}"
    );
}

#[test]
fn select_with_order_by_multiple_columns() {
    let schema = users_schema();
    let order_by = OrderBy {
        exprs: vec![
            OrderByExpr {
                expr: col(0, 1),
                order: Some(Direction::Asc),
            },
            OrderByExpr {
                expr: col(0, 0),
                order: Some(Direction::Desc),
            },
        ],
    };
    let stmt = make_query(Filter::ALL, Some(order_by), None, vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains(r#"ORDER BY tbl_0_0."name" ASC, tbl_0_0."id" DESC"#),
        "expected multi-column ORDER BY in: {sql}"
    );
}

#[test]
fn select_with_limit() {
    let schema = users_schema();
    let limit = Limit::Offset(LimitOffset {
        limit: Expr::from(10i64),
        offset: None,
    });
    let stmt = make_query(Filter::ALL, None, Some(limit), vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(sql.contains("LIMIT 10"), "expected `LIMIT 10` in: {sql}");
    assert!(!sql.contains("OFFSET"), "did not expect OFFSET in: {sql}");
}

#[test]
fn select_with_limit_and_offset() {
    let schema = users_schema();
    let limit = Limit::Offset(LimitOffset {
        limit: Expr::from(10i64),
        offset: Some(Expr::from(20i64)),
    });
    let stmt = make_query(Filter::ALL, None, Some(limit), vec![]);
    let sql = render(Flavor::Sqlite, &schema, stmt);
    assert!(
        sql.contains("LIMIT 10 OFFSET 20"),
        "expected `LIMIT 10 OFFSET 20` in: {sql}"
    );
}

#[test]
fn select_with_for_update() {
    let schema = users_schema();
    // SQLite has no row-level locks, but the serializer renders `FOR UPDATE`
    // unconditionally — verify that for all three flavors.
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let stmt = make_query(Filter::ALL, None, None, vec![Lock::Update]);
        let sql = render(flavor, &schema, stmt);
        assert!(
            sql.contains("FOR UPDATE"),
            "[{}] expected `FOR UPDATE` in: {sql}",
            flavor.name()
        );
    }
}

#[test]
fn select_with_for_share() {
    let schema = users_schema();
    // As with `FOR UPDATE`, the serializer renders `FOR SHARE` unconditionally
    // across flavors; database compatibility is enforced at execution, not here.
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        let stmt = make_query(Filter::ALL, None, None, vec![Lock::Share]);
        let sql = render(flavor, &schema, stmt);
        assert!(
            sql.contains("FOR SHARE"),
            "[{}] expected `FOR SHARE` in: {sql}",
            flavor.name()
        );
    }
}
