use toasty_core::schema::db::{Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId};
use toasty_core::schema::db::Type as DbType;
use toasty_core::stmt::{Expr, ExprColumn, ExprContext, ExprReference, Type};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn make_schema(col_types: &[(Type, &str)]) -> Schema {
    let table_id = TableId(0);

    let columns = col_types
        .iter()
        .enumerate()
        .map(|(i, (ty, name))| Column {
            id: ColumnId { table: table_id, index: i },
            name: name.to_string(),
            ty: ty.clone(),
            storage_ty: DbType::Text,
            nullable: false,
            primary_key: i == 0,
            auto_increment: false,
        })
        .collect();

    Schema {
        tables: vec![Table {
            id: table_id,
            name: "t".to_string(),
            columns,
            primary_key: PrimaryKey {
                columns: vec![ColumnId { table: table_id, index: 0 }],
                index: IndexId { table: table_id, index: 0 },
            },
            indices: vec![],
        }],
    }
}

fn cx<'a>(schema: &'a Schema) -> ExprContext<'a, Schema> {
    ExprContext::new_with_target(schema, &schema.tables[0])
}

fn col_ref(index: usize) -> Expr {
    Expr::Reference(ExprReference::Column(ExprColumn {
        nesting: 0,
        table: 0,
        column: index,
    }))
}

// ---------------------------------------------------------------------------
// Single column — each stmt::Type variant
// ---------------------------------------------------------------------------

#[test]
fn infer_reference_column_bool() {
    let s = make_schema(&[(Type::Bool, "flag")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::Bool);
}

#[test]
fn infer_reference_column_i64() {
    let s = make_schema(&[(Type::I64, "id")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::I64);
}

#[test]
fn infer_reference_column_u64() {
    let s = make_schema(&[(Type::U64, "count")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::U64);
}

#[test]
fn infer_reference_column_string() {
    let s = make_schema(&[(Type::String, "name")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::String);
}

#[test]
fn infer_reference_column_bytes() {
    let s = make_schema(&[(Type::Bytes, "data")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::Bytes);
}

#[test]
fn infer_reference_column_uuid() {
    let s = make_schema(&[(Type::Uuid, "id")]);
    assert_eq!(cx(&s).infer_expr_ty(&col_ref(0), &[]), Type::Uuid);
}

// ---------------------------------------------------------------------------
// Multiple columns — correct column selected by index
// ---------------------------------------------------------------------------

#[test]
fn infer_reference_selects_column_by_index() {
    let s = make_schema(&[
        (Type::I64, "id"),
        (Type::String, "name"),
        (Type::Bool, "active"),
    ]);
    let cx = cx(&s);
    assert_eq!(cx.infer_expr_ty(&col_ref(0), &[]), Type::I64);
    assert_eq!(cx.infer_expr_ty(&col_ref(1), &[]), Type::String);
    assert_eq!(cx.infer_expr_ty(&col_ref(2), &[]), Type::Bool);
}
