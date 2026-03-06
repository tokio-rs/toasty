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
    ResolvedRef, SourceTable, SourceTableId, TableDerived, TableFactor, TableRef, TableWithJoins,
    Type, Value, Values,
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
