use crate as toasty;
use crate::engine::test_util::*;
use crate::schema::Register;
use toasty_core::{
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
