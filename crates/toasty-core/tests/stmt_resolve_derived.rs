//! Tests for `resolve_expr_reference` when the target includes derived tables.
//!
//! Verifies that `ResolvedRef::Derived(DerivedRef { .. })` carries a reference
//! to the actual `TableDerived` along with the correct nesting level and column
//! index.

use toasty_core::schema::db::{
    Column, ColumnId, IndexId as DbIndexId, PrimaryKey as DbPrimaryKey, Schema as DbSchema, Table,
    TableId, Type as DbType,
};
use toasty_core::stmt::{
    DerivedRef, Expr, ExprColumn, ExprContext, ExprReference, ExprSet, ExprTarget, Query,
    ResolvedRef, Returning, Select, Source, SourceTable, SourceTableId, TableDerived, TableFactor,
    TableRef, TableWithJoins, Type, TypeUnion, Value, Values,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal `db::Schema` with one table (satisfies the `Resolve` trait).
fn db_schema() -> DbSchema {
    let table_id = TableId(0);
    DbSchema {
        tables: vec![Table {
            id: table_id,
            name: "t".into(),
            columns: vec![Column {
                id: ColumnId {
                    table: table_id,
                    index: 0,
                },
                name: "id".into(),
                ty: Type::I64,
                storage_ty: DbType::Integer(8),
                nullable: false,
                primary_key: true,
                auto_increment: false,
                versionable: false,
            }],
            primary_key: DbPrimaryKey {
                columns: vec![ColumnId {
                    table: table_id,
                    index: 0,
                }],
                index: DbIndexId {
                    table: table_id,
                    index: 0,
                },
            },
            indices: vec![],
        }],
    }
}

fn derived_from_values(rows: Vec<Expr>) -> TableDerived {
    TableDerived {
        subquery: Box::new(Query {
            with: None,
            body: ExprSet::Values(Values::new(rows)),
            single: false,
            order_by: None,
            limit: None,
            locks: vec![],
        }),
    }
}

fn source_with_derived(derived: TableDerived) -> SourceTable {
    SourceTable {
        tables: vec![TableRef::Derived(derived)],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    }
}

fn source_with_table(schema: &DbSchema) -> SourceTable {
    SourceTable {
        tables: vec![TableRef::Table(schema.tables[0].id)],
        from: vec![TableWithJoins {
            relation: TableFactor::Table(SourceTableId(0)),
            joins: vec![],
        }],
    }
}

fn col_ref(nesting: usize, table: usize, column: usize) -> ExprReference {
    ExprReference::Column(ExprColumn {
        nesting,
        table,
        column,
    })
}

fn val_row(values: Vec<Value>) -> Expr {
    Expr::record(values.into_iter().map(Expr::from).collect::<Vec<_>>())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_derived_returns_derived_ref() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(matches!(
        cx.resolve_expr_reference(&col_ref(0, 0, 0)),
        ResolvedRef::Derived(DerivedRef {
            nesting: 0,
            index: 0,
            ..
        })
    ));
}

#[test]
fn resolve_derived_preserves_column_index() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![val_row(vec![
        Value::I64(1),
        Value::String("a".into()),
    ])]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(matches!(
        cx.resolve_expr_reference(&col_ref(0, 0, 0)),
        ResolvedRef::Derived(DerivedRef { index: 0, .. })
    ));
    assert!(matches!(
        cx.resolve_expr_reference(&col_ref(0, 0, 1)),
        ResolvedRef::Derived(DerivedRef { index: 1, .. })
    ));
}

#[test]
fn derived_ref_provides_access_to_table_derived() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![val_row(vec![Value::Null])]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    let ResolvedRef::Derived(derived_ref) = cx.resolve_expr_reference(&col_ref(0, 0, 0)) else {
        panic!("expected Derived");
    };

    let ExprSet::Values(values) = &derived_ref.derived.subquery.body else {
        panic!("expected VALUES body");
    };
    assert_eq!(values.rows.len(), 1);
}

/// Mirrors the real EXISTS subquery pattern:
///   outer scope → real table (users)
///   inner scope → derived table from VALUES
///   inner filter references both via nesting
#[test]
fn nested_scopes_derived_inner_table_outer() {
    let schema = db_schema();
    let outer_source = source_with_table(&schema);
    let inner_source = source_with_derived(derived_from_values(vec![val_row(vec![Value::Null])]));

    let outer = ExprContext::new_with_target(&schema, ExprTarget::Source(&outer_source));
    let inner = outer.scope(ExprTarget::Source(&inner_source));

    // nesting=0 → inner derived table
    assert!(matches!(
        inner.resolve_expr_reference(&col_ref(0, 0, 0)),
        ResolvedRef::Derived(DerivedRef {
            nesting: 0,
            index: 0,
            ..
        })
    ));

    // nesting=1 → outer real table
    assert!(matches!(
        inner.resolve_expr_reference(&col_ref(1, 0, 0)),
        ResolvedRef::Column(_)
    ));
}

#[test]
fn infer_derived_values_columns() {
    let schema = db_schema();
    let values = vec![Value::I64(1), Value::String("hello".into())];

    for row in [
        val_row(values.clone()),
        Expr::from(Value::record_from_vec(values)),
    ] {
        let source = source_with_derived(derived_from_values(vec![row]));
        let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

        assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 0)), Type::I64);
        assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 1)), Type::String);
    }
}

#[test]
fn infer_derived_values_columns_uses_all_rows() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![
        val_row(vec![Value::Null, Value::I64(1), Value::Bool(true)]),
        val_row(vec![
            Value::String("hello".into()),
            Value::Null,
            Value::Bool(false),
        ]),
    ]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));
    let mut string_ty = TypeUnion::new();
    string_ty.insert(Type::Null);
    string_ty.insert(Type::String);
    let mut int_ty = TypeUnion::new();
    int_ty.insert(Type::I64);
    int_ty.insert(Type::Null);

    assert_eq!(
        cx.infer_expr_reference_ty(&col_ref(0, 0, 0)),
        string_ty.simplify()
    );
    assert_eq!(
        cx.infer_expr_reference_ty(&col_ref(0, 0, 1)),
        int_ty.simplify()
    );
    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 2)), Type::Bool);
}

#[test]
fn infer_derived_scalar_values_column() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![Expr::from("hello")]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 0)), Type::String);
}

#[test]
fn infer_derived_empty_values_columns() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 0)), Type::Unknown);
    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 1)), Type::Unknown);
}

#[test]
fn infer_derived_select_columns() {
    let schema = db_schema();
    let mut select = Select::from(schema.tables[0].id);
    select.returning = Returning::from_project_iter([
        Expr::from("hello"),
        Expr::from(col_ref(0, 0, 0)),
        Expr::list([Expr::from(1_i64)]),
    ]);
    let source = source_with_derived(TableDerived {
        subquery: Box::new(Query::new(select)),
    });
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 0)), Type::String);
    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 1)), Type::I64);
    assert_eq!(
        cx.infer_expr_reference_ty(&col_ref(0, 0, 2)),
        Type::list(Type::I64)
    );
}

#[test]
fn infer_derived_select_over_derived_values() {
    let schema = db_schema();
    let values = source_with_derived(derived_from_values(vec![val_row(vec![
        Value::I64(1),
        Value::String("hello".into()),
    ])]));
    let mut select = Select::new(Source::Table(values), true);
    select.returning = Returning::Project(col_ref(0, 0, 1).into());
    let source = source_with_derived(TableDerived {
        subquery: Box::new(Query::new(select)),
    });
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert_eq!(cx.infer_expr_reference_ty(&col_ref(0, 0, 0)), Type::String);
}

#[test]
fn infer_derived_column_with_argument_types() {
    let schema = db_schema();
    let values = source_with_derived(derived_from_values(vec![Expr::record([
        Expr::arg(0),
        Expr::arg(1),
    ])]));
    let mut select = Select::new(Source::Table(values.clone()), true);
    select.returning = Returning::from_project_iter([Expr::arg(0), Expr::from(col_ref(0, 0, 1))]);
    let projected = source_with_derived(TableDerived {
        subquery: Box::new(Query::new(select)),
    });

    for source in [values, projected] {
        let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));
        let args = [Type::String, Type::I64];

        assert_eq!(
            cx.infer_expr_ty(&col_ref(0, 0, 0).into(), &args),
            Type::String
        );
        assert_eq!(cx.infer_expr_ty(&col_ref(0, 0, 1).into(), &args), Type::I64);
    }
}

#[test]
fn infer_derived_column_from_nested_scope() {
    let schema = db_schema();
    let outer_source = source_with_derived(derived_from_values(vec![Expr::from("hello")]));
    let outer = ExprContext::new_with_target(&schema, ExprTarget::Source(&outer_source));

    // The derived SELECT refers to its own table and the scope outside its owner.
    let mut select = Select::from(schema.tables[0].id);
    select.returning = Returning::from_project_iter([col_ref(0, 0, 0), col_ref(2, 0, 0)]);
    let source = source_with_derived(TableDerived {
        subquery: Box::new(Query::new(select)),
    });
    let owner = outer.scope(ExprTarget::Source(&source));
    let inner_source = source_with_table(&schema);
    let inner = owner.scope(ExprTarget::Source(&inner_source));

    assert_eq!(inner.infer_expr_reference_ty(&col_ref(1, 0, 0)), Type::I64);
    assert_eq!(
        inner.infer_expr_reference_ty(&col_ref(1, 0, 1)),
        Type::String
    );
}

// ---------------------------------------------------------------------------
// DerivedRef::is_column_always_null
// ---------------------------------------------------------------------------

fn resolve_derived<'a>(
    cx: &'a ExprContext<'a, DbSchema>,
    col: &'a ExprReference,
) -> DerivedRef<'a> {
    match cx.resolve_expr_reference(col) {
        ResolvedRef::Derived(d) => d,
        other => panic!("expected Derived, got {other:?}"),
    }
}

#[test]
fn is_column_always_null_single_null_row() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![val_row(vec![Value::Null])]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
}

#[test]
fn is_column_always_null_multiple_null_rows() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![
        val_row(vec![Value::Null]),
        val_row(vec![Value::Null]),
    ]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
}

#[test]
fn is_column_always_null_false_when_non_null() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![val_row(vec![Value::I64(42)])]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(!resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
}

#[test]
fn is_column_always_null_false_when_mixed() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![
        val_row(vec![Value::Null]),
        val_row(vec![Value::I64(42)]),
    ]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(!resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
}

#[test]
fn is_column_always_null_false_when_empty_values() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    assert!(!resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
}

#[test]
fn is_column_always_null_checks_correct_column() {
    let schema = db_schema();
    let source = source_with_derived(derived_from_values(vec![val_row(vec![
        Value::I64(1),
        Value::Null,
    ])]));
    let cx = ExprContext::new_with_target(&schema, ExprTarget::Source(&source));

    // Column 0 is non-null
    assert!(!resolve_derived(&cx, &col_ref(0, 0, 0)).is_column_always_null());
    // Column 1 is null
    assert!(resolve_derived(&cx, &col_ref(0, 0, 1)).is_column_always_null());
}
