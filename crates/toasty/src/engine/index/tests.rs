use super::*;
use toasty_core::{
    schema::{app, Builder},
    stmt,
};

use crate as toasty;
use crate::model::Register;

#[allow(dead_code)]
#[derive(toasty::Model)]
struct User {
    #[key]
    id: String,
    name: String,
}

/// Composite primary key: user_id (partition) + status (sort).
#[allow(dead_code)]
#[derive(toasty::Model)]
struct Todo {
    #[key]
    user_id: String,
    #[key]
    status: String,
}

#[test]
fn pk_equality_goes_to_index_filter() -> Result<()> {
    let cx = sqlite_test_cx();

    // col[0] = 1  — pk column equality
    let filter = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(1i64)),
    );

    let plan = cx.plan_basic_query_with_filter(filter.clone())?;

    assert!(
        plan.index.primary_key,
        "should select the primary key index"
    );
    assert_eq!(
        plan.index_filter, filter,
        "pk equality should be the index filter"
    );
    assert!(
        plan.result_filter.is_none(),
        "no residual result filter expected"
    );
    assert!(plan.post_filter.is_none());
    Ok(())
}

#[test]
fn and_splits_pk_to_index_and_name_to_result() -> Result<()> {
    let cx = sqlite_test_cx();

    // col[0] = 1 AND col[1] = 2 — pk equality AND name equality
    let pk_eq = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(1i64)),
    );
    let name_eq = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 1)),
        stmt::Expr::Value(stmt::Value::from(2i64)),
    );
    let filter = stmt::Expr::And(stmt::ExprAnd {
        operands: vec![pk_eq.clone(), name_eq.clone()],
    });

    let plan = cx.plan_basic_query_with_filter(filter)?;

    assert!(plan.index.primary_key);
    assert_eq!(
        plan.index_filter, pk_eq,
        "only the pk condition goes to index filter"
    );
    assert_eq!(
        plan.result_filter.as_ref(),
        Some(&name_eq),
        "non-pk condition goes to result filter"
    );
    Ok(())
}

#[test]
fn or_on_pk_stays_as_or_for_sql() -> Result<()> {
    let cx = sqlite_test_cx(); // SQLite — index_or_predicate = true

    // col[0] = 1 OR col[0] = 2
    let pk_eq_1 = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(1i64)),
    );
    let pk_eq_2 = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(2i64)),
    );
    let filter = stmt::Expr::Or(stmt::ExprOr {
        operands: vec![pk_eq_1, pk_eq_2],
    });

    let plan = cx.plan_basic_query_with_filter(filter.clone())?;

    assert!(plan.index.primary_key);
    assert_eq!(
        plan.index_filter, filter,
        "OR should be preserved as-is for SQL backends"
    );
    assert!(plan.result_filter.is_none());
    assert!(plan.post_filter.is_none());
    Ok(())
}

#[test]
fn or_on_pk_becomes_any_map_for_dynamodb() -> Result<()> {
    let cx = ddb_test_cx();

    // col[0] = 1 OR col[0] = 2
    let pk_eq_1 = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(1i64)),
    );
    let pk_eq_2 = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
        stmt::Expr::Value(stmt::Value::from(2i64)),
    );
    let filter = stmt::Expr::Or(stmt::ExprOr {
        operands: vec![pk_eq_1, pk_eq_2],
    });

    let plan = cx.plan_basic_query_with_filter(filter)?;

    // Expected: ANY(MAP([1, 2], col[0][0] = arg(0)))
    let expected = stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::Value(stmt::Value::List(vec![
            stmt::Value::from(1i64),
            stmt::Value::from(2i64),
        ])),
        stmt::Expr::eq(
            stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
            stmt::Expr::arg(0),
        ),
    ));

    assert!(plan.index.primary_key);
    assert_eq!(
        plan.index_filter, expected,
        "OR should be rewritten to ANY(MAP(...)) for DynamoDB"
    );
    assert!(plan.result_filter.is_none());
    assert!(plan.post_filter.is_none());
    Ok(())
}

#[test]
fn and_with_any_map_distributes_into_any_map_for_dynamodb() -> Result<()> {
    // Schema: Todo { user_id (pk partition), status (pk sort) }
    // Both columns are part of the primary key index, so both land in index_filter.
    //
    // Filter: col[0][1] = "active" AND ANY(MAP(arg[0], col[0][0] = arg[0]))
    //   col[0][0] = user_id (partition key), col[0][1] = status (sort key)
    //
    // Expected: ANY(MAP(arg[0], col[0][0] = arg[0] AND col[0][1] = "active"))
    let cx = ddb_test_cx_composite();

    let status_eq = stmt::Expr::eq(
        stmt::Expr::Reference(stmt::ExprReference::column(0, 1)),
        stmt::Expr::Value(stmt::Value::String("active".to_string())),
    );
    let user_id_any = stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::arg(0),
        stmt::Expr::eq(
            stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
            stmt::Expr::arg(0),
        ),
    ));
    let filter = stmt::Expr::And(stmt::ExprAnd {
        operands: vec![status_eq.clone(), user_id_any],
    });

    let plan = cx.plan_basic_query_with_filter(filter)?;

    let expected = stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::arg(0),
        stmt::Expr::And(stmt::ExprAnd {
            operands: vec![
                stmt::Expr::eq(
                    stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
                    stmt::Expr::arg(0),
                ),
                status_eq,
            ],
        }),
    ));

    assert!(plan.index.primary_key);
    assert_eq!(
        plan.index_filter, expected,
        "AND with ANY(MAP) should distribute the non-Any operands into the map predicate"
    );
    assert!(plan.result_filter.is_none());
    assert!(plan.post_filter.is_none());
    Ok(())
}

#[test]
fn any_map_with_arg_base_passes_through_for_dynamodb() {
    // ANY(MAP(arg[0], col[0][0] = arg[0])) — the batch-load canonical form produced by
    // rewrite_stmt_query_for_batch_load_nosql. It is already in the target form so
    // index_filter_to_any_map must return it unchanged.
    let filter = stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::arg(0),
        stmt::Expr::eq(
            stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
            stmt::Expr::arg(0),
        ),
    ));

    let result = or_rewrite::index_filter_to_any_map(filter.clone());
    assert_eq!(result, filter, "batch-load ANY(MAP(arg, pred)) should pass through unchanged");
}

#[test]
#[ignore = "extract_shape for composite-key AND branches is not yet implemented"]
fn composite_pk_or_becomes_any_map_for_dynamodb() -> Result<()> {
    // Schema: Todo { user_id (pk partition), status (pk sort) }
    //
    // Filter: (user_id = "u1" AND status = "s1") OR (user_id = "u2" AND status = "s2")
    //
    // Expected: ANY(MAP(
    //     [(u1,s1), (u2,s2)],
    //     user_id = arg(0) AND status = arg(1)
    // ))
    let cx = ddb_test_cx_composite();

    let branch = |uid: &str, status: &str| {
        stmt::Expr::And(stmt::ExprAnd {
            operands: vec![
                stmt::Expr::eq(
                    stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
                    stmt::Expr::Value(stmt::Value::String(uid.to_string())),
                ),
                stmt::Expr::eq(
                    stmt::Expr::Reference(stmt::ExprReference::column(0, 1)),
                    stmt::Expr::Value(stmt::Value::String(status.to_string())),
                ),
            ],
        })
    };
    let filter = stmt::Expr::Or(stmt::ExprOr {
        operands: vec![branch("u1", "s1"), branch("u2", "s2")],
    });

    let plan = cx.plan_basic_query_with_filter(filter)?;

    let record = |uid: &str, status: &str| {
        stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
            stmt::Value::String(uid.to_string()),
            stmt::Value::String(status.to_string()),
        ]))
    };
    let expected = stmt::Expr::any(stmt::Expr::map(
        stmt::Expr::Value(stmt::Value::List(vec![record("u1", "s1"), record("u2", "s2")])),
        stmt::Expr::And(stmt::ExprAnd {
            operands: vec![
                stmt::Expr::eq(
                    stmt::Expr::Reference(stmt::ExprReference::column(0, 0)),
                    stmt::Expr::arg(0),
                ),
                stmt::Expr::eq(
                    stmt::Expr::Reference(stmt::ExprReference::column(0, 1)),
                    stmt::Expr::arg(1),
                ),
            ],
        }),
    ));

    assert!(plan.index.primary_key);
    assert_eq!(
        plan.index_filter, expected,
        "composite OR should be rewritten to ANY(MAP([records], col0=arg(0) AND col1=arg(1)))"
    );
    assert!(plan.result_filter.is_none());
    assert!(plan.post_filter.is_none());
    Ok(())
}

struct TestCx {
    schema: toasty_core::Schema,
    capability: &'static Capability,
}

fn sqlite_test_cx() -> TestCx {
    test_cx_with_capability(&Capability::SQLITE)
}

fn ddb_test_cx() -> TestCx {
    test_cx_with_capability(&Capability::DYNAMODB)
}

fn ddb_test_cx_composite() -> TestCx {
    let app_schema =
        app::Schema::from_macro(&[Todo::schema()]).expect("schema should build from macro");
    let schema = Builder::new()
        .build(app_schema, &Capability::DYNAMODB)
        .expect("schema should build");
    TestCx { schema, capability: &Capability::DYNAMODB }
}

fn test_cx_with_capability(capability: &'static Capability) -> TestCx {
    let app_schema =
        app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");
    let schema = Builder::new()
        .build(app_schema, capability)
        .expect("schema should build");
    TestCx { schema, capability }
}

impl TestCx {
    /// Build a table-targeting `SELECT` statement with the given filter against
    /// the first table in the schema.  This mirrors the lowered statements that
    /// reach `plan_index_path` at runtime.
    fn basic_query_with_filter(&self, filter: stmt::Expr) -> stmt::Statement {
        let table_id = self.schema.db.tables[0].id;
        let source = stmt::SourceTable::new(
            vec![stmt::TableRef::Table(table_id)],
            stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins: vec![],
            },
        );
        stmt::Statement::Query(stmt::Query {
            with: None,
            body: stmt::ExprSet::Select(Box::new(stmt::Select::new(source, filter))),
            single: false,
            order_by: None,
            limit: None,
            locks: vec![],
        })
    }

    fn plan_basic_query_with_filter(&self, filter: stmt::Expr) -> Result<IndexPlan<'_>> {
        let stmt = self.basic_query_with_filter(filter);
        plan_index_path(&self.schema, self.capability, &stmt)
    }
}
