//! Tests for `lower::returning::constantize_update_returning`.

use crate as toasty;
use crate::engine::lower::returning::constantize_update_returning;
use crate::engine::test_util::test_schema_with;
use crate::schema::{Embed, Model};
use toasty_core::stmt::{self, Expr, Returning, Value};

#[derive(Debug, PartialEq, toasty::Embed)]
struct Profile {
    name: String,
    age: i64,
}

#[derive(Debug, toasty::Model)]
#[allow(dead_code)]
struct Account {
    #[key]
    id: i64,
    #[document]
    profile: Profile,
}

/// An update that sets a `#[document]` field constantizes its returning: the
/// projection reads the column through the mapping's raising cast, the
/// assignment carries the lowering cast over the set value, and evaluating
/// the pair round-trips to the positional record — a schema-directed
/// conversion, so the evaluation must have schema access.
#[test]
fn document_update_returning_constantizes() {
    let schema = test_schema_with(&[Account::schema(), Profile::schema()]);

    let (column_id, doc_ty) = schema
        .mapping
        .document_columns
        .first()
        .expect("the schema has one document column");
    let column = schema.db.column(*column_id);
    let table = schema.db.table(column_id.table);

    // The lowered shapes, straight from the mapping: the returning projection
    // is the document field's raising expression (`Cast(col, Model)`), and
    // the assignment is the `model_to_table` lowering cast with the set value
    // substituted in — exactly what `lower_set_assignment` emits.
    let mapping = schema.mapping.models.values().next().unwrap();
    let raising_expr = mapping
        .fields
        .iter()
        .filter_map(|field| field.as_primitive())
        .find(|prim| prim.column == *column_id)
        .expect("the document field maps to a primitive")
        .column_expr
        .clone();

    let profile = Value::record_from_vec(vec![Value::from("new"), Value::I64(99)]);
    let mut assignments = stmt::Assignments::new();
    assignments.set(
        *column_id,
        Expr::cast_from(Expr::Value(profile.clone()), doc_ty, &column.ty),
    );

    let mut returning = Returning::Project(Expr::record_from_vec(vec![raising_expr]));

    constantize_update_returning(
        stmt::ExprContext::new_with_target(&schema, table),
        &mut returning,
        &assignments,
    );

    // Lowering cast then raising cast round-trip to the positional record.
    assert_eq!(
        returning,
        Returning::Project(Expr::Value(Value::record_from_vec(vec![profile])))
    );
}
