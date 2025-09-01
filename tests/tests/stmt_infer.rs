use tests::{models, tests, DbTest};
use toasty::{stmt::Id, Model};
use toasty_core::stmt::{
    Assignments, Delete, Expr, ExprFunc, ExprSet, FuncCount, Insert,
    InsertTarget, Query, Returning, Select, Source, SourceModel, Statement, Type, Update,
    UpdateTarget, Value, Values,
};

/// Simple test models
#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,
    age: i32,
}

/// Test basic Statement::infer_ty delegation to Query
async fn test_statement_query_delegation(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    // Create a simple SELECT * query
    let query = Query {
        body: ExprSet::Select(Box::new(Select {
            source: Source::Model(SourceModel {
                model: User::id(),
                include: Default::default(),
                via: None,
            }),
            returning: Returning::Star,
            filter: true.into(),
        })),
        with: None,
        order_by: None,
        limit: None,
        locks: vec![],
    };

    let statement = Statement::Query(query);

    // Test that Statement delegates to Query and returns List(Model(user_model_id))
    let inferred_type = statement.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Query::infer_ty returns list and has proper debug assertion
async fn test_query_returns_list_type(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let query = Query {
        body: ExprSet::Select(Box::new(Select {
            source: Source::Model(SourceModel {
                model: User::id(),
                include: Default::default(),
                via: None,
            }),
            returning: Returning::Star,
            filter: true.into(),
        })),
        with: None,
        order_by: None,
        limit: None,
        locks: vec![],
    };

    let inferred_type = query.infer_ty(schema, &[]);

    // Verify it's a list type (this tests the debug assertion in Query::infer_ty)
    assert!(inferred_type.is_list());
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Select with Returning::Star on Model source
async fn test_select_star_model_source(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let select = Select {
        source: Source::Model(SourceModel {
            model: User::id(),
            include: Default::default(),
            via: None,
        }),
        returning: Returning::Star,
        filter: true.into(),
    };

    let inferred_type = select.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Select with Returning::Expr
async fn test_select_returning_expr(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let select = Select {
        source: Source::Model(SourceModel {
            model: User::id(),
            include: Default::default(),
            via: None,
        }),
        returning: Returning::Expr("test".into()),
        filter: true.into(),
    };

    let inferred_type = select.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(Type::String));
}

/// Test Values with non-empty rows
async fn test_values_with_rows(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let values = Values {
        rows: vec![42_i64.into(), 43_i64.into()],
    };

    let inferred_type = values.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(Type::I64));
}

/// Test Values with empty rows returns Unknown
async fn test_values_empty_returns_unknown(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let inferred_type = Values { rows: vec![] }.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Unknown);
}

/// Test Insert with Returning::Star and Model target
async fn test_insert_returning_star(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let insert = Insert {
        target: InsertTarget::Model(User::id()),
        source: Query {
            body: ExprSet::Values(Values { rows: vec![] }),
            with: None,
            order_by: None,
            limit: None,
            locks: vec![],
        },
        returning: Some(Returning::Star),
    };

    let inferred_type = insert.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Insert with no returning clause
async fn test_insert_no_returning(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let insert = Insert {
        target: InsertTarget::Model(User::id()),
        source: Query {
            body: ExprSet::Values(Values { rows: vec![] }),
            with: None,
            order_by: None,
            limit: None,
            locks: vec![],
        },
        returning: None,
    };

    let inferred_type = insert.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Null);
}

/// Test Update with Returning::Star and Model target
async fn test_update_returning_star(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let update = Update {
        target: UpdateTarget::Model(User::id()),
        assignments: Assignments::default(),
        filter: None,
        condition: None,
        returning: Some(Returning::Star),
    };

    let inferred_type = update.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Update with Returning::Changed returns SparseRecord
async fn test_update_returning_changed(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let mut assignments = Assignments::default();
    assignments.set(1, "new_name");
    assignments.set(2, 25_i64);

    let update = Update {
        target: UpdateTarget::Model(User::id()),
        assignments,
        filter: None,
        condition: None,
        returning: Some(Returning::Changed),
    };

    let inferred_type = update.infer_ty(schema, &[]);

    // Should be List(SparseRecord) with field set containing keys 1 and 2
    match inferred_type {
        Type::List(inner) => match *inner {
            Type::SparseRecord(field_set) => {
                assert!(field_set.contains(1usize));
                assert!(field_set.contains(2usize));
                assert_eq!(field_set.len(), 2);
            }
            _ => panic!("Expected SparseRecord, got {:?}", inner),
        },
        _ => panic!("Expected List type, got {:?}", inferred_type),
    }
}

/// Test Delete with Returning::Star and Model source
async fn test_delete_returning_star(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let delete = Delete {
        from: Source::Model(SourceModel {
            model: User::id(),
            include: Default::default(),
            via: None,
        }),
        filter: true.into(),
        returning: Some(Returning::Star),
    };

    let inferred_type = delete.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(User::id()));
}

/// Test Expr::Arg with valid index
async fn test_expr_arg_valid_index(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let arg_expr = Expr::Arg(0.into());
    let args = vec![Type::String, Type::I32];

    let inferred_type = arg_expr.infer_ty(schema, &args);
    assert_eq!(inferred_type, Type::String);
}

/// Test Expr::Value for basic types
async fn test_expr_value_types(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let test_cases = vec![
        (true.into(), Type::Bool),
        (42_i32.into(), Type::I32),
        ("test".into(), Type::String),
        (Value::Null, Type::Null),
    ];

    for (value, expected_type) in test_cases {
        let inferred_type = value.infer_ty(schema, &[]);
        assert_eq!(inferred_type, expected_type);
    }
}

/// Test ExprFunc::Count returns I64
async fn test_expr_func_count(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let count_func = ExprFunc::Count(FuncCount {
        arg: None,
        filter: None,
    });

    let inferred_type = count_func.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::I64);
}

/// Test Expr::List with items
async fn test_expr_list_with_items(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let list_expr = Expr::list_from_vec(vec![
        "a".into(),
        "b".into(),
    ]);

    let inferred_type = list_expr.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(Type::String));
}

/// Test Expr::List empty returns Unknown
async fn test_expr_list_empty_returns_unknown(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let empty_list = Expr::list_from_vec(vec![]);

    let inferred_type = empty_list.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Unknown);
}

/// Test Expr::Record
async fn test_expr_record(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let record_expr = Expr::record_from_vec(vec![
        "test".into(),
        42_i64.into(),
    ]);

    let inferred_type = record_expr.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Record(vec![Type::String, Type::I64]));
}

/// Test Value::Record
async fn test_value_record(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let record_value = Value::record_from_vec(vec!["test".into(), 42_i64.into()]);

    let inferred_type = record_value.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Record(vec![Type::String, Type::I64]));
}

/// Test Value::List with items
async fn test_value_list_with_items(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let list_value = Value::list_from_vec(vec![1_i64.into(), 2_i64.into(), 3_i64.into()]);

    let inferred_type = list_value.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::list(Type::I64));
}

/// Test Value::List empty returns Unknown
async fn test_value_list_empty_returns_unknown(test: &mut DbTest) {
    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    let empty_list = Value::list_from_vec(vec![]);

    let inferred_type = empty_list.infer_ty(schema, &[]);
    assert_eq!(inferred_type, Type::Unknown);
}

tests! {
    test_statement_query_delegation,
    test_query_returns_list_type,
    test_select_star_model_source,
    test_select_returning_expr,
    test_values_with_rows,
    test_values_empty_returns_unknown,
    test_insert_returning_star,
    test_insert_no_returning,
    test_update_returning_star,
    test_update_returning_changed,
    test_delete_returning_star,
    test_expr_arg_valid_index,
    test_expr_value_types,
    test_expr_func_count,
    test_expr_list_with_items,
    test_expr_list_empty_returns_unknown,
    test_expr_record,
    test_value_record,
    test_value_list_with_items,
    test_value_list_empty_returns_unknown,
}
