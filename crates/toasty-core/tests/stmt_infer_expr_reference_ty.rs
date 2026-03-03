//! Type-inference tests for `Expr::Reference`, covering all three
//! `ExprReference` variants (Column, Field, Model) in each relevant
//! `ExprTarget` context, at both nesting=0 and nesting>0.

use toasty_core::schema::app::IndexId as AppIndexId;
use toasty_core::schema::app::{
    Field, FieldName, FieldPrimitive, FieldTy, ModelId, ModelRoot, PrimaryKey as AppPrimaryKey,
};
use toasty_core::schema::db::Type as DbType;
use toasty_core::schema::db::{
    Column, ColumnId, IndexId as DbIndexId, PrimaryKey as DbPrimaryKey, Schema, Table, TableId,
};
use toasty_core::schema::Name;
use toasty_core::stmt::{Expr, ExprColumn, ExprContext, ExprReference, ExprTarget, Type};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build a `db::Schema` with a single table whose columns have the given types.
fn db_schema(col_types: &[(Type, &str)]) -> Schema {
    let table_id = TableId(0);
    let columns = col_types
        .iter()
        .enumerate()
        .map(|(i, (ty, name))| Column {
            id: ColumnId {
                table: table_id,
                index: i,
            },
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

/// `ExprContext` targeting the first (and only) table in a `db::Schema`.
fn table_cx(schema: &Schema) -> ExprContext<'_, Schema> {
    ExprContext::new_with_target(schema, &schema.tables[0])
}

/// Build a `ModelRoot` whose fields have the given expression types.
fn model_root(id: usize, field_types: &[(Type, &str)]) -> ModelRoot {
    let model_id = ModelId(id);
    let fields = field_types
        .iter()
        .enumerate()
        .map(|(i, (ty, name))| Field {
            id: model_id.field(i),
            name: FieldName {
                app_name: name.to_string(),
                storage_name: None,
            },
            ty: FieldTy::Primitive(FieldPrimitive {
                ty: ty.clone(),
                storage_ty: None,
            }),
            nullable: false,
            primary_key: i == 0,
            auto: None,
            constraints: vec![],
        })
        .collect();

    ModelRoot {
        id: model_id,
        name: Name::new("T"),
        fields,
        primary_key: AppPrimaryKey {
            fields: vec![model_id.field(0)],
            index: AppIndexId {
                model: model_id,
                index: 0,
            },
        },
        table_name: None,
        indices: vec![],
    }
}

/// `ExprContext<()>` targeting a model. Field/Model references don't need
/// schema lookups, so the schema-less `()` resolver suffices.
fn model_cx(model: &ModelRoot) -> ExprContext<'_, ()> {
    ExprContext::new_with_target(&(), model)
}

fn col_ref(nesting: usize, column: usize) -> Expr {
    Expr::Reference(ExprReference::Column(ExprColumn {
        nesting,
        table: 0,
        column,
    }))
}

fn field_ref(nesting: usize, index: usize) -> Expr {
    Expr::Reference(ExprReference::Field { nesting, index })
}

fn model_ref(nesting: usize) -> Expr {
    Expr::Reference(ExprReference::Model { nesting })
}

// ---------------------------------------------------------------------------
// ExprReference::Column  ×  ExprTarget::Table
// ---------------------------------------------------------------------------

#[test]
fn column_nesting0_bool() {
    let s = db_schema(&[(Type::Bool, "flag")]);
    assert_eq!(table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]), Type::Bool);
}

#[test]
fn column_nesting0_i64() {
    let s = db_schema(&[(Type::I64, "id")]);
    assert_eq!(table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]), Type::I64);
}

#[test]
fn column_nesting0_u64() {
    let s = db_schema(&[(Type::U64, "count")]);
    assert_eq!(table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]), Type::U64);
}

#[test]
fn column_nesting0_string() {
    let s = db_schema(&[(Type::String, "name")]);
    assert_eq!(
        table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]),
        Type::String
    );
}

#[test]
fn column_nesting0_bytes() {
    let s = db_schema(&[(Type::Bytes, "data")]);
    assert_eq!(table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]), Type::Bytes);
}

#[test]
fn column_nesting0_uuid() {
    let s = db_schema(&[(Type::Uuid, "id")]);
    assert_eq!(table_cx(&s).infer_expr_ty(&col_ref(0, 0), &[]), Type::Uuid);
}

#[test]
fn column_nesting0_selected_by_index() {
    let s = db_schema(&[
        (Type::I64, "id"),
        (Type::String, "name"),
        (Type::Bool, "active"),
    ]);
    let cx = table_cx(&s);
    assert_eq!(cx.infer_expr_ty(&col_ref(0, 0), &[]), Type::I64);
    assert_eq!(cx.infer_expr_ty(&col_ref(0, 1), &[]), Type::String);
    assert_eq!(cx.infer_expr_ty(&col_ref(0, 2), &[]), Type::Bool);
}

/// nesting=1: a child scope's column reference resolves against the parent table.
#[test]
fn column_nesting1_resolves_from_parent() {
    let s = db_schema(&[(Type::I64, "id"), (Type::String, "name")]);
    let parent = table_cx(&s);
    let child = parent.scope(ExprTarget::Free);

    assert_eq!(child.infer_expr_ty(&col_ref(1, 0), &[]), Type::I64);
    assert_eq!(child.infer_expr_ty(&col_ref(1, 1), &[]), Type::String);
}

/// nesting=2: a grandchild scope's column reference resolves against the grandparent.
#[test]
fn column_nesting2_resolves_from_grandparent() {
    let s = db_schema(&[(Type::Bool, "flag")]);
    let grandparent = table_cx(&s);
    let parent = grandparent.scope(ExprTarget::Free);
    let child = parent.scope(ExprTarget::Free);

    assert_eq!(child.infer_expr_ty(&col_ref(2, 0), &[]), Type::Bool);
}

// ---------------------------------------------------------------------------
// ExprReference::Field  ×  ExprTarget::Model
// ---------------------------------------------------------------------------

#[test]
fn field_nesting0_first_field() {
    let m = model_root(0, &[(Type::I64, "id")]);
    assert_eq!(model_cx(&m).infer_expr_ty(&field_ref(0, 0), &[]), Type::I64);
}

#[test]
fn field_nesting0_selected_by_index() {
    let m = model_root(
        0,
        &[
            (Type::I64, "id"),
            (Type::String, "name"),
            (Type::Bool, "active"),
        ],
    );
    let cx = model_cx(&m);
    assert_eq!(cx.infer_expr_ty(&field_ref(0, 0), &[]), Type::I64);
    assert_eq!(cx.infer_expr_ty(&field_ref(0, 1), &[]), Type::String);
    assert_eq!(cx.infer_expr_ty(&field_ref(0, 2), &[]), Type::Bool);
}

#[test]
fn field_nesting0_uuid() {
    let m = model_root(0, &[(Type::Uuid, "id"), (Type::String, "email")]);
    let cx = model_cx(&m);
    assert_eq!(cx.infer_expr_ty(&field_ref(0, 0), &[]), Type::Uuid);
    assert_eq!(cx.infer_expr_ty(&field_ref(0, 1), &[]), Type::String);
}

/// nesting=1: a child scope's field reference resolves against the parent model.
#[test]
fn field_nesting1_resolves_from_parent() {
    let m = model_root(0, &[(Type::I64, "id"), (Type::String, "name")]);
    let parent = model_cx(&m);
    let child = parent.scope(ExprTarget::Free);

    assert_eq!(child.infer_expr_ty(&field_ref(1, 0), &[]), Type::I64);
    assert_eq!(child.infer_expr_ty(&field_ref(1, 1), &[]), Type::String);
}

// ---------------------------------------------------------------------------
// ExprReference::Model  ×  ExprTarget::Model
// ---------------------------------------------------------------------------

/// nesting=0: returns Type::Model(model.id).
#[test]
fn model_ref_nesting0() {
    let m = model_root(0, &[(Type::I64, "id")]);
    assert_eq!(
        model_cx(&m).infer_expr_ty(&model_ref(0), &[]),
        Type::Model(ModelId(0))
    );
}

/// Different model IDs produce different Type::Model values.
#[test]
fn model_ref_nesting0_distinct_ids() {
    let m0 = model_root(0, &[(Type::I64, "id")]);
    let m1 = model_root(1, &[(Type::Uuid, "id")]);
    assert_eq!(
        model_cx(&m0).infer_expr_ty(&model_ref(0), &[]),
        Type::Model(ModelId(0))
    );
    assert_eq!(
        model_cx(&m1).infer_expr_ty(&model_ref(0), &[]),
        Type::Model(ModelId(1))
    );
}

/// nesting=1: a child scope's model reference resolves against the parent model.
#[test]
fn model_ref_nesting1_resolves_from_parent() {
    let m = model_root(42, &[(Type::I64, "id")]);
    let parent = model_cx(&m);
    let child = parent.scope(ExprTarget::Free);

    assert_eq!(
        child.infer_expr_ty(&model_ref(1), &[]),
        Type::Model(ModelId(42))
    );
}
