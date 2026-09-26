use crate as toasty;
use crate::engine::test_util::*;
use crate::schema::{Embed, Model};
use toasty_core::{
    schema::db,
    stmt::{self, Expr, Value},
};

use super::extract::{binary_like_prefix_pattern, glob_prefix_pattern};
use super::{Param, Ty, run};

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
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

    assert_eq!(params.len(), 2);
    assert_eq!(params[0].value, Value::from("abc"));
    assert_eq!(params[1].value, Value::from("hello"));

    // Statement should have Arg placeholders
    if let stmt::Statement::Insert(insert) = &stmt
        && let stmt::ExprSet::Values(values) = &insert.source.body
    {
        let row = &values.rows[0];
        assert!(matches!(row, Expr::Record(_)));
        if let Expr::Record(record) = row {
            assert!(matches!(record.fields[0], Expr::Arg(_)));
            assert!(matches!(record.fields[1], Expr::Arg(_)));
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
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

    // Only 'abc' should be extracted; NULL stays as literal
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].value, Value::from("abc"));
}

// ============================================================================
// Multi-row INSERT → unnest transpose (Capability::insert_values_unnest)
// ============================================================================

/// Build `INSERT INTO items (id, name) VALUES ('a1', 'n1'), ('a2', 'n2')`.
fn two_row_item_insert(schema: &toasty_core::Schema) -> stmt::Statement {
    stmt::Statement::Insert(stmt::Insert {
        target: insert_target(schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a1"),
                Value::from("n1"),
            ]))),
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a2"),
                Value::from("n2"),
            ]))),
        ])),
        upsert: None,
        returning: None,
    })
}

fn unnest_source(stmt: &stmt::Statement) -> &[stmt::ExprFunc] {
    let stmt::Statement::Insert(insert) = stmt else {
        panic!("expected insert");
    };
    let stmt::ExprSet::Select(select) = &insert.source.body else {
        panic!("expected select source");
    };
    assert!(matches!(select.returning, stmt::Returning::Star));
    let source = select.source.as_table_unwrap();
    let [stmt::TableRef::RowsFrom(funcs)] = source.tables.as_slice() else {
        panic!("expected ROWS FROM table source");
    };
    funcs
}

#[test]
fn multi_row_insert_transposed_to_column_arrays_on_postgres() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_postgresql(&[Item::schema()]);
    let mut stmt = two_row_item_insert(&schema);

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::POSTGRESQL,
    );

    // One array param per column, each holding that column's values across rows.
    assert_eq!(params.len(), 2);
    assert_eq!(
        params[0].value,
        Value::List(vec![Value::from("a1"), Value::from("a2")])
    );
    assert_eq!(
        params[1].value,
        Value::List(vec![Value::from("n1"), Value::from("n2")])
    );

    assert_eq!(params[0].ty, db::Type::list(db::Type::Text));
    assert_eq!(params[1].ty, db::Type::list(db::Type::Text));

    // ROWS FROM evaluates one single-argument unnest function per column.
    let funcs = unnest_source(&stmt);
    let [
        stmt::ExprFunc::Unnest(first),
        stmt::ExprFunc::Unnest(second),
    ] = funcs
    else {
        panic!("expected two unnest functions");
    };
    assert!(matches!(first.arg.as_ref(), Expr::Arg(_)));
    assert!(matches!(second.arg.as_ref(), Expr::Arg(_)));
}

#[test]
fn multi_row_insert_with_null_cell_binds_one_array_per_column() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: Option<String>,
    }

    let schema = test_schema_postgresql(&[Item::schema()]);
    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a1"),
                Value::from("n1"),
            ]))),
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a2"),
                Value::Null,
            ]))),
        ])),
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::POSTGRESQL,
    );

    // The NULL cell rides inside the column's array param; it must not decay
    // to an inline NULL literal (which would break the unnest arity).
    assert_eq!(params.len(), 2);
    assert_eq!(
        params[1].value,
        Value::List(vec![Value::from("n1"), Value::Null])
    );
    assert_eq!(params[1].ty, db::Type::list(db::Type::Text));

    let [_, stmt::ExprFunc::Unnest(unnest)] = unnest_source(&stmt) else {
        panic!("expected two unnest functions");
    };
    assert!(matches!(unnest.arg.as_ref(), Expr::Arg(_)));
}

#[test]
fn multi_row_insert_all_null_column_typed_from_schema() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: Option<String>,
    }

    let schema = test_schema_postgresql(&[Item::schema()]);
    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a1"),
                Value::Null,
            ]))),
            Expr::from(Value::Record(stmt::ValueRecord::from_vec(vec![
                Value::from("a2"),
                Value::Null,
            ]))),
        ])),
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::POSTGRESQL,
    );

    // An all-NULL column array carries no value to infer from; its element
    // type must come from the target column's storage type.
    assert_eq!(params.len(), 2);
    assert_eq!(params[1].value, Value::List(vec![Value::Null, Value::Null]));
    assert_eq!(params[1].ty, db::Type::list(db::Type::Text));
}

#[test]
fn multi_row_insert_not_transposed_without_capability() {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[Item::schema()]);
    let mut stmt = two_row_item_insert(&schema);

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

    // SQLite keeps the per-cell VALUES form with four scalar params.
    assert_eq!(params.len(), 4);
    let stmt::Statement::Insert(insert) = &stmt else {
        panic!("expected insert");
    };
    let stmt::ExprSet::Values(values) = &insert.source.body else {
        panic!("expected values body");
    };
    assert_eq!(values.rows.len(), 2);
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
    // but we can test that bind handles an empty query
    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );
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
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

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
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

    assert_eq!(params.len(), 2);
    assert!(matches!(&params[0].ty, db::Type::Text));
    assert!(matches!(&params[1].ty, db::Type::Integer(8)));
}

#[test]
fn synthesize_unnest_returns_array_element_type() {
    use super::infer::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![Param {
        value: Value::List(vec![Value::from("a"), Value::from("b")]),
        ty: Ty::List(Box::new(Ty::Inferred(db::Type::Text))),
    }];
    let expr = Expr::Func(
        stmt::FuncUnnest {
            arg: Box::new(Expr::arg(0)),
        }
        .into(),
    );

    let ty = synthesize(&expr, &cx, &mut params);

    assert!(matches!(ty, Ty::Inferred(db::Type::Text)));
}

#[test]
fn synthesize_unnest_returns_array_column_element_type() {
    use super::infer::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![Param {
        value: Value::List(vec![Value::from("a"), Value::from("b")]),
        ty: Ty::Column(db::Type::list(db::Type::Text)),
    }];
    let expr = Expr::Func(
        stmt::FuncUnnest {
            arg: Box::new(Expr::arg(0)),
        }
        .into(),
    );

    let ty = synthesize(&expr, &cx, &mut params);

    assert!(matches!(ty, Ty::Column(db::Type::Text)));
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

    use super::infer::synthesize;

    let schema = test_schema();

    // Directly test the synthesize function with a constructed expression
    // that has a multi-step projection
    let mut params = vec![
        Param {
            value: Value::from("a"),
            ty: Ty::Inferred(db::Type::Text),
        },
        Param {
            value: Value::from(1i32),
            ty: Ty::Inferred(db::Type::Integer(4)),
        },
        Param {
            value: Value::from(true),
            ty: Ty::Inferred(db::Type::Boolean),
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
        matches!(ty, Ty::Inferred(db::Type::Integer(4))),
        "expected Inferred(Integer(4)), got {:?}",
        ty
    );
}

// ============================================================================
// Invalid state panics
// ============================================================================

#[test]
#[should_panic(expected = "index out of bounds")]
fn synthesize_arg_out_of_range_panics() {
    use super::infer::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![];

    // Arg(5) but params is empty
    synthesize(&Expr::arg(5), &cx, &mut params);
}

#[test]
#[should_panic(expected = "out of range")]
fn synthesize_project_step_out_of_bounds_panics() {
    use super::infer::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![Param {
        value: Value::from("a"),
        ty: Ty::Inferred(db::Type::Text),
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
    use super::infer::synthesize;

    let schema = test_schema();
    let cx = stmt::ExprContext::new(&schema.db);
    let mut params = vec![Param {
        value: Value::from(42i64),
        ty: Ty::Inferred(db::Type::Integer(8)),
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
    use super::infer::merge;

    // Record vs Inferred scalar
    let a = Ty::Record(vec![Ty::Inferred(db::Type::Text)]);
    let b = Ty::Inferred(db::Type::Integer(8));

    merge(&a, &b);
}

#[test]
#[should_panic(expected = "incompatible")]
fn merge_records_different_lengths_panics() {
    use super::infer::merge;

    let a = Ty::Record(vec![Ty::Inferred(db::Type::Text)]);
    let b = Ty::Record(vec![
        Ty::Inferred(db::Type::Text),
        Ty::Inferred(db::Type::Integer(8)),
    ]);

    merge(&a, &b);
}

#[test]
fn merge_column_wins_over_inferred() {
    use super::infer::merge;

    let col = Ty::Column(db::Type::Enum(db::TypeEnum {
        name: Some("status".to_string()),
        variants: vec![],
    }));
    let inferred = Ty::Inferred(db::Type::Text);

    // Column should win regardless of argument order
    let result = merge(&col, &inferred);
    assert!(result.is_column(), "expected Column, got {:?}", result);

    let result = merge(&inferred, &col);
    assert!(result.is_column(), "expected Column, got {:?}", result);
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

// ============================================================================
// glob_prefix_pattern
// ============================================================================

#[test]
fn glob_plain_prefix() {
    assert_eq!(glob_prefix_pattern("alpha"), "alpha*");
}

#[test]
fn glob_empty_prefix() {
    assert_eq!(glob_prefix_pattern(""), "*");
}

#[test]
fn glob_escapes_star() {
    assert_eq!(glob_prefix_pattern("100*off"), "100[*]off*");
}

#[test]
fn glob_escapes_question_mark() {
    assert_eq!(glob_prefix_pattern("a?b"), "a[?]b*");
}

#[test]
fn glob_escapes_open_bracket() {
    assert_eq!(glob_prefix_pattern("a[b"), "a[[]b*");
}

#[test]
fn glob_like_wildcards_are_literal() {
    // `%` and `_` are LIKE wildcards but not GLOB metacharacters.
    assert_eq!(glob_prefix_pattern("100%"), "100%*");
    assert_eq!(glob_prefix_pattern("a_b"), "a_b*");
}

// ============================================================================
// binary_like_prefix_pattern
// ============================================================================

#[test]
fn binary_like_plain_prefix() {
    assert_eq!(binary_like_prefix_pattern("alpha"), "alpha%");
}

#[test]
fn binary_like_empty_prefix() {
    assert_eq!(binary_like_prefix_pattern(""), "%");
}

#[test]
fn binary_like_escapes_percent() {
    assert_eq!(binary_like_prefix_pattern("100%"), "100!%%");
}

#[test]
fn binary_like_escapes_underscore() {
    assert_eq!(binary_like_prefix_pattern("a_b"), "a!_b%");
}

#[test]
fn binary_like_escapes_escape_char() {
    assert_eq!(binary_like_prefix_pattern("!bang"), "!!bang%");
}

#[test]
fn binary_like_glob_wildcards_are_literal() {
    // `*` and `?` are GLOB metacharacters but not LIKE metacharacters.
    assert_eq!(binary_like_prefix_pattern("foo*"), "foo*%");
    assert_eq!(binary_like_prefix_pattern("a?b"), "a?b%");
}

// ============================================================================
// Expr::Static survives binding
// ============================================================================

#[test]
fn static_value_in_insert_row_is_not_extracted() {
    // Mixed row: one user-supplied `Expr::Value` (extracted) and one
    // synthetic `Expr::Static` (passes through). Confirms the bind pass
    // keys on the variant, not the value content.
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[Item::schema()]);

    let mut stmt = stmt::Statement::Insert(stmt::Insert {
        target: insert_target(&schema, "items"),
        source: stmt::Query::values(stmt::Values::new(vec![Expr::Record(stmt::ExprRecord {
            fields: vec![
                Expr::Static(Value::from("static-key")),
                Expr::Value(Value::from("user-name")),
            ],
        })])),
        upsert: None,
        returning: None,
    });

    let params = run(
        &mut stmt,
        &schema.db,
        &toasty_core::driver::Capability::SQLITE,
    );

    // Only the `Expr::Value` is extracted; the `Expr::Static` passes through.
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].value, Value::from("user-name"));

    let stmt::Statement::Insert(insert) = &stmt else {
        unreachable!()
    };
    let stmt::ExprSet::Values(values) = &insert.source.body else {
        unreachable!()
    };
    let Expr::Record(record) = &values.rows[0] else {
        unreachable!()
    };
    assert!(
        matches!(&record.fields[0], Expr::Static(Value::String(s)) if s == "static-key"),
        "Static leaf should survive binding, got {:#?}",
        record.fields[0]
    );
    assert!(
        matches!(&record.fields[1], Expr::Arg(_)),
        "Value leaf should be extracted to Arg, got {:#?}",
        record.fields[1]
    );
}
