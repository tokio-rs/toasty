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

#[test]
fn pk_equality_goes_to_index_filter() -> Result<()> {
    let cx = test_cx();

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
    let cx = test_cx();

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

struct TestCx {
    schema: toasty_core::Schema,
    capability: &'static Capability,
}

fn test_cx() -> TestCx {
    let app_schema =
        app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");
    let schema = Builder::new()
        .build(app_schema, &Capability::DYNAMODB)
        .expect("schema should build");

    TestCx {
        schema,
        capability: &Capability::SQLITE,
    }
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
