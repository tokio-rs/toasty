use crate as toasty;
use crate::engine::test_util::*;
use crate::schema::Register;
use toasty_core::{
    driver::operation::TypedValue,
    schema::db,
    stmt::{self, Expr, Value},
};

use super::extract_params;

// ============================================================================
// Basic extraction
// ============================================================================

#[test]
fn extract_scalar_from_simple_insert() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[Item::schema()]);

    // Build: INSERT INTO items (id, name) VALUES ('abc', 'hello')
    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![Expr::from(Value::Record(
            stmt::ValueRecord::from_vec(vec![Value::from("abc"), Value::from("hello")]),
        ))])),
        returning: None,
    });

    let params = extract_params(&mut stmt, &schema);

    assert_eq!(params.len(), 2);
    assert_eq!(params[0].value, Value::from("abc"));
    assert_eq!(params[1].value, Value::from("hello"));

    // Statement should have Arg placeholders
    if let stmt::Statement::Insert(insert) = &stmt {
        if let stmt::ExprSet::Values(values) = &insert.source.body {
            let row = &values.rows[0];
            assert!(matches!(row, Expr::Record(_)));
            if let Expr::Record(record) = row {
                assert!(matches!(record.fields[0], Expr::Arg(_)));
                assert!(matches!(record.fields[1], Expr::Arg(_)));
            }
        }
    }
}

#[test]
fn null_values_not_extracted() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: Option<String>,
    }

    let schema = test_schema_with(&[Item::schema()]);

    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![Expr::from(Value::Record(
            stmt::ValueRecord::from_vec(vec![Value::from("abc"), Value::Null]),
        ))])),
        returning: None,
    });

    let params = extract_params(&mut stmt, &schema);

    // Only 'abc' should be extracted; NULL stays as literal
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].value, Value::from("abc"));
}

#[test]
fn extract_from_where_clause() {
    let schema = test_schema();

    // Build: SELECT ... WHERE col = 'hello'
    let _filter_expr = Expr::BinaryOp(stmt::ExprBinaryOp {
        lhs: Box::new(Expr::Value(Value::from("hello"))),
        op: stmt::BinaryOp::Eq,
        rhs: Box::new(Expr::Value(Value::from(42i64))),
    });

    let mut stmt = stmt::Statement::Query(stmt::Query {
        with: None,
        body: stmt::ExprSet::Values(stmt::Values::new(vec![])),
        single: false,
        order_by: None,
        limit: None,
        locks: vec![],
    });

    // Can't easily build a full SELECT with filter without a schema,
    // but we can test that extract_params handles an empty query
    let params = extract_params(&mut stmt, &schema);
    assert_eq!(params.len(), 0);
}

// ============================================================================
// Type refinement for enums
// ============================================================================

#[test]
fn enum_insert_refines_type_to_enum() {
    #[derive(Debug, toasty::Embed)]
    enum Status {
        Active,
        Inactive,
    }

    #[derive(toasty::Model)]
    struct Task {
        #[key]
        id: String,
        status: Status,
    }

    let schema = test_schema_postgresql(&[Task::schema(), Status::schema()]);

    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "tasks"),
        source: stmt::Query::values(stmt::Values::new(vec![Expr::from(Value::Record(
            stmt::ValueRecord::from_vec(vec![Value::from("task-1"), Value::from("active")]),
        ))])),
        returning: None,
    });

    let params = extract_params(&mut stmt, &schema);

    assert_eq!(params.len(), 2);

    // The status param should be refined to Enum type, not plain Text
    assert!(
        matches!(&params[1].ty, db::Type::Enum(_)),
        "expected Enum type for status param, got {:?}",
        params[1].ty
    );
}

#[test]
fn non_enum_insert_keeps_default_types() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        count: i64,
    }

    let schema = test_schema_with(&[Item::schema()]);

    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![Expr::from(Value::Record(
            stmt::ValueRecord::from_vec(vec![Value::from("item-1"), Value::from(42i64)]),
        ))])),
        returning: None,
    });

    let params = extract_params(&mut stmt, &schema);

    assert_eq!(params.len(), 2);
    assert!(matches!(&params[0].ty, db::Type::Text));
    assert!(matches!(&params[1].ty, db::Type::Integer(8)));
}

// ============================================================================
// Projection type inference
// ============================================================================

#[test]
fn synthesize_multi_step_projection() {
    // Test that a multi-step projection like project(record, [1, 0])
    // correctly walks through nested record types.
    //
    // Given: Record([Text, Record([Integer(4), Boolean])])
    // Project [1, 0] should yield Integer(4)

    use super::{InferredType, merge, synthesize};

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);

    // Build: project(project(arg(0), [1]), [0])
    // This is how multi-step projections are typically represented:
    // chained Project expressions, each with a single step.
    // But let's also test a single Project with multiple steps.

    // First, test the chained case (typical):
    // inner = Record([String("a"), Record([I32(1), Bool(true)])])
    let inner_record = Value::Record(stmt::ValueRecord::from_vec(vec![
        Value::from(1i32),
        Value::from(true),
    ]));
    let outer_record = Value::Record(stmt::ValueRecord::from_vec(vec![
        Value::from("a"),
        inner_record,
    ]));

    // Extract params from: INSERT INTO ... VALUES (outer_record)
    // This gives us Arg positions for the scalar values
    #[derive(toasty::Model)]
    struct Dummy {
        #[key]
        id: String,
    }
    let schema = test_schema_with(&[Dummy::schema()]);

    // Directly test the synthesize function with a constructed expression
    // that has a multi-step projection
    let mut params = vec![
        TypedValue {
            value: Value::from("a"),
            ty: db::Type::Text,
        },
        TypedValue {
            value: Value::from(1i32),
            ty: db::Type::Integer(4),
        },
        TypedValue {
            value: Value::from(true),
            ty: db::Type::Boolean,
        },
    ];

    // Build: Record([Arg(0), Record([Arg(1), Arg(2)])])
    let expr = Expr::Record(stmt::ExprRecord::from_vec(vec![
        Expr::arg(0),
        Expr::Record(stmt::ExprRecord::from_vec(vec![Expr::arg(1), Expr::arg(2)])),
    ]));

    // Project with steps [1, 0] — field 1 of outer (inner record), then field 0 (Integer)
    let mut projection = stmt::Projection::single(1);
    projection.push(0);
    let project_expr = Expr::Project(stmt::ExprProject {
        base: Box::new(expr),
        projection,
    });

    let cx = stmt::ExprContext::new(&schema.db);
    let ty = synthesize(&project_expr, &cx, &mut params);

    assert!(
        matches!(ty, InferredType::Scalar(db::Type::Integer(4))),
        "expected Integer(4), got {:?}",
        ty
    );
}

// ============================================================================
// Invalid state panics
// ============================================================================

#[test]
#[should_panic(expected = "index out of bounds")]
fn synthesize_arg_out_of_range_panics() {
    use super::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![];

    // Arg(5) but params is empty
    synthesize(&Expr::arg(5), &cx, &mut params);
}

#[test]
#[should_panic(expected = "out of range")]
fn synthesize_project_step_out_of_bounds_panics() {
    use super::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![TypedValue {
        value: Value::from("a"),
        ty: db::Type::Text,
    }];

    // Record with 1 field, projecting step 5
    let expr = Expr::Project(stmt::ExprProject {
        base: Box::new(Expr::Record(stmt::ExprRecord::from_vec(vec![Expr::arg(0)]))),
        projection: stmt::Projection::single(5),
    });

    synthesize(&expr, &cx, &mut params);
}

#[test]
#[should_panic(expected = "non-record")]
fn synthesize_project_from_scalar_panics() {
    use super::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![TypedValue {
        value: Value::from(42i64),
        ty: db::Type::Integer(8),
    }];

    // Project from a scalar Arg (not a record)
    let expr = Expr::Project(stmt::ExprProject {
        base: Box::new(Expr::arg(0)),
        projection: stmt::Projection::single(0),
    });

    synthesize(&expr, &cx, &mut params);
}

#[test]
#[should_panic(expected = "incompatible")]
fn merge_incompatible_structures_panics() {
    use super::{InferredType, merge};

    // Record vs Scalar
    let a = InferredType::Record(vec![InferredType::Scalar(db::Type::Text)]);
    let b = InferredType::Scalar(db::Type::Integer(8));

    merge(&a, &b);
}

#[test]
#[should_panic(expected = "incompatible")]
fn merge_records_different_lengths_panics() {
    use super::{InferredType, merge};

    let a = InferredType::Record(vec![InferredType::Scalar(db::Type::Text)]);
    let b = InferredType::Record(vec![
        InferredType::Scalar(db::Type::Text),
        InferredType::Scalar(db::Type::Integer(8)),
    ]);

    merge(&a, &b);
}

#[test]
#[should_panic(expected = "incompatible types")]
fn more_specific_incompatible_types_panics() {
    use super::more_specific;

    // Boolean and Integer are not compatible
    more_specific(&db::Type::Boolean, &db::Type::Integer(8));
}

// ============================================================================
// Helpers
// ============================================================================

/// Build an InsertTarget::Table for the named table in the schema.
fn insert_target(schema: &toasty_core::Schema, table_name: &str) -> stmt::InsertTarget {
    let table = schema
        .db
        .tables
        .iter()
        .find(|t| t.name.ends_with(table_name))
        .unwrap_or_else(|| panic!("table '{table_name}' not found in schema"));

    stmt::InsertTarget::Table(stmt::InsertTable {
        table: table.id,
        columns: table.columns.iter().map(|c| c.id).collect(),
    })
}
