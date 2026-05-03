//! Extract typed bind parameters from a fully-resolved statement.
//!
//! Three phases:
//! 1. **Extract**: Mechanically replace scalar `Value` nodes with `Arg(n)`
//!    placeholders, initializing each param's type from the value itself.
//! 2. **Synthesize** (bottom-up): Compute the inferred type of each expression
//!    node from its children (column refs get their storage type from the schema,
//!    records get a tuple of field types, etc.).
//! 3. **Check** (top-down): Push refined types down into `Arg(n)` nodes,
//!    upgrading param types when context provides more precise information
//!    (e.g., `Enum` instead of `Text`).
//!
//! Synthesize and check happen together in a single recursive walk: each node
//! synthesizes its children first, then comparison operators merge both sides
//! and check them against the merged type.
//!
//! Types carry **provenance** (`Column` vs `Inferred`) so that schema-
//! authoritative column types always win over value-inferred guesses during
//! merging.

use toasty_core::{
    driver::operation::TypedValue,
    schema::{Schema, db},
    stmt,
};

/// Expression context bound to the database schema.
type Cx<'a> = stmt::ExprContext<'a, db::Schema>;

// ============================================================================
// Public entry point
// ============================================================================

/// Extract bind parameters from a statement, replacing scalar values with
/// `Expr::Arg(n)` placeholders and inferring precise `db::Type` for each.
pub(crate) fn extract_params(stmt: &mut stmt::Statement, schema: &Schema) -> Vec<TypedValue> {
    // Phase 1: Mechanical extraction — replace values with Arg(n)
    let mut params = Vec::new();
    extract_values(stmt, &mut params);

    // Phase 2+3: Bidirectional type inference — refine param types
    refine_param_types(stmt, &schema.db, &mut params);

    params
}

// ============================================================================
// Inferred type representation
// ============================================================================

/// The inferred database-level type of an expression node.
///
/// Each scalar type carries **provenance**: `Column` means the type came from
/// the schema (authoritative), `Inferred` means it was guessed from the value.
/// Column types always win when merging.
#[derive(Debug, Clone)]
enum Ty {
    /// Type from a column reference or schema (authoritative).
    Column(db::Type),
    /// Type inferred from a value (initial guess — may be less specific).
    Inferred(db::Type),
    /// A tuple of types (one per field).
    Record(Vec<Ty>),
    /// A homogeneous list where all elements share a type.
    List(Box<Ty>),
    /// Type could not be determined.
    Unknown,
}

impl Ty {
    /// Extract the `db::Type`, regardless of provenance.
    fn db_type(&self) -> Option<&db::Type> {
        match self {
            Ty::Column(ty) | Ty::Inferred(ty) => Some(ty),
            _ => None,
        }
    }

    /// Returns true if this type comes from the schema (authoritative).
    fn is_column(&self) -> bool {
        matches!(self, Ty::Column(_))
    }
}

// ============================================================================
// Phase 1: Mechanical value extraction
// ============================================================================

/// Replace all scalar `Value` nodes with `Arg(n)` placeholders.
/// Initialize each param's `ty` from the value itself.
fn extract_values(stmt: &mut stmt::Statement, params: &mut Vec<TypedValue>) {
    // Pre-pass: `IN <list>` predicates expect their rhs to render as a tuple
    // of placeholders (`x IN (?1, ?2, ?3)`), so a `Value::List` rhs must be
    // unrolled before extraction. Without this, `is_extractable_scalar` would
    // bundle the whole list as a single parameter — correct for collection
    // columns but wrong here.
    unroll_in_list_value_lists(stmt);

    stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
        match expr {
            // Scalar value → extract
            stmt::Expr::Value(value) if is_extractable_scalar(value) => {
                let ty = db::Type::from_value(value);
                let position = params.len();
                let value = std::mem::replace(value, stmt::Value::Null);
                params.push(TypedValue { value, ty });
                *expr = stmt::Expr::arg(position);
            }

            // Value::Record or Value::List → take ownership, convert to
            // Expr::Record/Expr::List with extracted fields
            stmt::Expr::Value(value @ (stmt::Value::Record(_) | stmt::Value::List(_))) => {
                let owned = std::mem::replace(value, stmt::Value::Null);
                *expr = value_to_extracted_expr(owned, params);
            }

            // Null, Default, and everything else: leave as-is
            _ => {}
        }
    });
}

/// Recursively convert a `Value` into an `Expr`, extracting scalar values.
/// Takes ownership to avoid cloning.
fn value_to_extracted_expr(value: stmt::Value, params: &mut Vec<TypedValue>) -> stmt::Expr {
    match value {
        stmt::Value::Null => stmt::Expr::Value(stmt::Value::Null),
        stmt::Value::Record(record) => {
            let fields = record
                .fields
                .into_iter()
                .map(|f| value_to_extracted_expr(f, params))
                .collect();
            stmt::Expr::Record(stmt::ExprRecord::from_vec(fields))
        }
        // A list of all-scalars is extracted as a single bind parameter so
        // that collection columns receive the whole list as one value
        // (rather than rendering as a tuple of placeholders).
        stmt::Value::List(values) if values.iter().all(is_extractable_scalar) => {
            let value = stmt::Value::List(values);
            let ty = db::Type::from_value(&value);
            let position = params.len();
            params.push(TypedValue { value, ty });
            stmt::Expr::arg(position)
        }
        stmt::Value::List(values) => {
            let items = values
                .into_iter()
                .map(|v| value_to_extracted_expr(v, params))
                .collect();
            stmt::Expr::List(stmt::ExprList { items })
        }
        scalar => {
            let ty = db::Type::from_value(&scalar);
            let position = params.len();
            params.push(TypedValue { value: scalar, ty });
            stmt::Expr::arg(position)
        }
    }
}

/// Walk the statement and rewrite any `Expr::InList { list }` whose `list` is
/// a `Expr::Value(Value::List(items))` into `Expr::InList { list: Expr::List(items as Expr::Value) }`.
/// Subsequent extraction then decomposes each item as its own scalar parameter,
/// producing `IN (?1, ?2, …)` rather than a single bundled parameter.
fn unroll_in_list_value_lists(stmt: &mut stmt::Statement) {
    stmt::visit_mut::for_each_expr_mut(stmt, |expr| {
        if let stmt::Expr::InList(in_list) = expr
            && let stmt::Expr::Value(stmt::Value::List(_)) = in_list.list.as_ref()
        {
            let list_expr =
                std::mem::replace(in_list.list.as_mut(), stmt::Expr::Value(stmt::Value::Null));
            let stmt::Expr::Value(stmt::Value::List(items)) = list_expr else {
                unreachable!()
            };
            *in_list.list = stmt::Expr::List(stmt::ExprList {
                items: items.into_iter().map(stmt::Expr::Value).collect(),
            });
        }
    });
}

fn is_extractable_scalar(value: &stmt::Value) -> bool {
    match value {
        stmt::Value::Null | stmt::Value::Record(_) => false,
        // A list of scalars is itself a single bind parameter — collection
        // columns (e.g. PostgreSQL `text[]` or a JSON-encoded list) take the
        // whole list as one driver-level value rather than expanding to a
        // tuple of placeholders. Lists containing records still decompose
        // (batch INSERT VALUES, etc.).
        stmt::Value::List(items) => items.iter().all(is_extractable_scalar),
        _ => true,
    }
}

// ============================================================================
// Phase 2+3: Bidirectional type inference
// ============================================================================

/// Refine param types by walking the statement with synthesize + check.
fn refine_param_types(stmt: &stmt::Statement, db_schema: &db::Schema, params: &mut [TypedValue]) {
    let cx = stmt::ExprContext::new(db_schema);
    refine_stmt(stmt, &cx, db_schema, params);
}

fn refine_stmt(
    stmt: &stmt::Statement,
    cx: &Cx<'_>,
    db_schema: &db::Schema,
    params: &mut [TypedValue],
) {
    match stmt {
        stmt::Statement::Insert(insert) => {
            let cx = cx.scope(insert);
            refine_insert(insert, &cx, db_schema, params);
        }
        stmt::Statement::Update(update) => {
            let cx = cx.scope(update);
            refine_update(update, &cx, db_schema, params);
        }
        stmt::Statement::Delete(delete) => {
            let cx = cx.scope(delete);
            refine_filter(&delete.filter, &cx, params);
        }
        stmt::Statement::Query(query) => {
            refine_query(query, cx, params);
        }
    }
}

fn refine_insert(
    insert: &stmt::Insert,
    _cx: &Cx<'_>,
    db_schema: &db::Schema,
    params: &mut [TypedValue],
) {
    // Build expected type from column list (authoritative)
    let expected = match &insert.target {
        stmt::InsertTarget::Table(table) => {
            let db_table = &db_schema.tables[table.table.0];
            let field_types: Vec<Ty> = table
                .columns
                .iter()
                .map(|col_id| Ty::Column(db_table.columns[col_id.index].storage_ty.clone()))
                .collect();
            Ty::Record(field_types)
        }
        _ => Ty::Unknown,
    };

    // Push column types down into each VALUES row
    if let stmt::ExprSet::Values(values) = &insert.source.body {
        for row in &values.rows {
            check(row, &expected, params);
        }
    }
}

fn refine_update(
    update: &stmt::Update,
    cx: &Cx<'_>,
    db_schema: &db::Schema,
    params: &mut [TypedValue],
) {
    // Refine assignment types from target columns
    if let stmt::UpdateTarget::Table(table_id) = &update.target {
        let db_table = &db_schema.tables[table_id.0];

        for (projection, assignment) in update.assignments.iter() {
            if let stmt::Assignment::Set(expr) = assignment {
                let steps = projection.as_slice();
                assert_eq!(
                    steps.len(),
                    1,
                    "UPDATE assignment projection should be a single column index, got {steps:?}"
                );
                let col_idx = steps[0];
                if let Some(col) = db_table.columns.get(col_idx) {
                    let expected = Ty::Column(col.storage_ty.clone());
                    check(expr, &expected, params);
                }
            }
        }
    }

    // Refine filter types
    refine_filter(&update.filter, cx, params);
}

fn refine_query(query: &stmt::Query, cx: &Cx<'_>, params: &mut [TypedValue]) {
    let cx = cx.scope(query);

    match &query.body {
        stmt::ExprSet::Select(select) => {
            let cx = cx.scope(&**select);
            refine_filter(&select.filter, &cx, params);
        }
        stmt::ExprSet::Values(values) => {
            for row in &values.rows {
                synthesize(row, &cx, params);
            }
        }
        _ => {}
    }

    // Handle CTEs
    if let Some(with) = &query.with {
        for cte in &with.ctes {
            refine_query(&cte.query, &cx, params);
        }
    }
}

fn refine_filter(filter: &stmt::Filter, cx: &Cx<'_>, params: &mut [TypedValue]) {
    if let Some(expr) = &filter.expr {
        synthesize(expr, cx, params);
    }
}

// ============================================================================
// Synthesize (bottom-up) — returns the inferred type with provenance
// ============================================================================

/// Compute the inferred type of an expression from its children.
///
/// For comparison operators, this also triggers `check()` to push refined
/// types down into both sides (bidirectional inference).
fn synthesize(expr: &stmt::Expr, cx: &Cx<'_>, params: &mut [TypedValue]) -> Ty {
    match expr {
        // Arg — type comes from the extracted param (inferred from value)
        stmt::Expr::Arg(arg) => {
            let tv = &params[arg.position];
            Ty::Inferred(tv.ty.clone())
        }

        // Column reference — authoritative from schema
        stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Column(_)) => {
            match cx.resolve_expr_reference(expr_ref) {
                stmt::ResolvedRef::Column(col) => Ty::Column(col.storage_ty.clone()),
                _ => Ty::Unknown,
            }
        }

        // Projection — walk each step to reach the projected field's type
        stmt::Expr::Project(project) => {
            let mut ty = synthesize(&project.base, cx, params);
            for &step in project.projection.as_slice() {
                ty = match ty {
                    Ty::Record(fields) => {
                        assert!(
                            step < fields.len(),
                            "projection step {step} out of range for record with {} fields",
                            fields.len()
                        );
                        fields.into_iter().nth(step).unwrap()
                    }
                    other => panic!("cannot project from non-record type: {other:?}"),
                };
            }
            ty
        }

        // Record — synthesize each field
        stmt::Expr::Record(record) => {
            let fields: Vec<Ty> = record
                .fields
                .iter()
                .map(|f| synthesize(f, cx, params))
                .collect();
            Ty::Record(fields)
        }

        // List — synthesize each item, merge to a common type
        stmt::Expr::List(list) => {
            let mut merged = Ty::Unknown;
            for item in &list.items {
                let item_ty = synthesize(item, cx, params);
                merged = merge(&merged, &item_ty);
            }
            Ty::List(Box::new(merged))
        }

        // BinaryOp (comparison) — synthesize both sides, merge, check both
        stmt::Expr::BinaryOp(binary) => {
            let lhs_ty = synthesize(&binary.lhs, cx, params);
            let rhs_ty = synthesize(&binary.rhs, cx, params);
            let merged = merge(&lhs_ty, &rhs_ty);
            check(&binary.lhs, &merged, params);
            check(&binary.rhs, &merged, params);
            Ty::Inferred(db::Type::Boolean)
        }

        // InList — synthesize expr, check list items against it
        stmt::Expr::InList(in_list) => {
            let expr_ty = synthesize(&in_list.expr, cx, params);
            synthesize(&in_list.list, cx, params);
            check_list(&in_list.list, &expr_ty, params);
            Ty::Inferred(db::Type::Boolean)
        }

        // InSubquery — synthesize the expression, recurse into subquery
        stmt::Expr::InSubquery(in_sub) => {
            synthesize(&in_sub.expr, cx, params);
            refine_query(&in_sub.query, cx, params);
            Ty::Inferred(db::Type::Boolean)
        }

        // Exists — recurse into subquery
        stmt::Expr::Exists(exists) => {
            refine_query(&exists.subquery, cx, params);
            Ty::Inferred(db::Type::Boolean)
        }

        // Nested statement
        stmt::Expr::Stmt(expr_stmt) => {
            refine_stmt(&expr_stmt.stmt, cx, cx.schema(), params);
            Ty::Unknown
        }

        // Logical operators — recurse, return boolean
        stmt::Expr::And(and) => {
            for op in &and.operands {
                synthesize(op, cx, params);
            }
            Ty::Inferred(db::Type::Boolean)
        }
        stmt::Expr::Or(or) => {
            for op in &or.operands {
                synthesize(op, cx, params);
            }
            Ty::Inferred(db::Type::Boolean)
        }
        stmt::Expr::Not(not) => {
            synthesize(&not.expr, cx, params);
            Ty::Inferred(db::Type::Boolean)
        }
        stmt::Expr::IsNull(is_null) => {
            synthesize(&is_null.expr, cx, params);
            Ty::Inferred(db::Type::Boolean)
        }

        // StartsWith — both sides are strings. Reaches here only on drivers
        // that natively support it (e.g., DynamoDB); SQL drivers lower it to
        // Like during the lowering phase.
        stmt::Expr::StartsWith(e) => {
            check(&e.expr, &Ty::Inferred(db::Type::Text), params);
            check(&e.prefix, &Ty::Inferred(db::Type::Text), params);
            Ty::Inferred(db::Type::Boolean)
        }

        // Like — both sides are strings
        stmt::Expr::Like(e) => {
            check(&e.expr, &Ty::Inferred(db::Type::Text), params);
            check(&e.pattern, &Ty::Inferred(db::Type::Text), params);
            Ty::Inferred(db::Type::Boolean)
        }

        // Values that weren't extracted (Null, Default)
        stmt::Expr::Value(stmt::Value::Null) => Ty::Unknown,
        stmt::Expr::Default => Ty::Unknown,

        // Anything else
        _ => Ty::Unknown,
    }
}

// ============================================================================
// Check (top-down) — pushes refined types into Arg nodes
// ============================================================================

/// Push an expected type down into an expression. When it reaches `Arg(n)`,
/// update `params[n].ty` if the expected type has column provenance.
fn check(expr: &stmt::Expr, expected: &Ty, params: &mut [TypedValue]) {
    match (expr, expected) {
        // Arg — update the param's type if expected has column provenance
        (stmt::Expr::Arg(arg), ty) if ty.is_column() => {
            if let Some(db_ty) = ty.db_type() {
                params[arg.position].ty = db_ty.clone();
            }
        }

        // Record — check each field against its expected type
        (stmt::Expr::Record(record), Ty::Record(field_types)) => {
            for (field, field_ty) in record.fields.iter().zip(field_types) {
                check(field, field_ty, params);
            }
        }

        // List — check each item against the expected element type
        (stmt::Expr::List(list), Ty::List(elem_ty)) => {
            for item in &list.items {
                check(item, elem_ty, params);
            }
        }
        (stmt::Expr::List(list), ty) if ty.db_type().is_some() => {
            // Scalar expected for each item (e.g., from InList)
            for item in &list.items {
                check(item, ty, params);
            }
        }

        // For other nodes, no downward propagation needed
        _ => {}
    }
}

/// Check all items in a list expression against an expected element type.
fn check_list(list_expr: &stmt::Expr, elem_ty: &Ty, params: &mut [TypedValue]) {
    match list_expr {
        stmt::Expr::List(list) => {
            for item in &list.items {
                check(item, elem_ty, params);
            }
        }
        _ => {
            check(list_expr, elem_ty, params);
        }
    }
}

// ============================================================================
// Merge — combines two types, column provenance wins
// ============================================================================

/// Merge two inferred types. Column provenance wins over Inferred.
fn merge(a: &Ty, b: &Ty) -> Ty {
    match (a, b) {
        (Ty::Unknown, other) | (other, Ty::Unknown) => other.clone(),

        // Both are scalars — column provenance wins
        (Ty::Column(a_ty), Ty::Column(b_ty)) => {
            assert_eq!(
                a_ty, b_ty,
                "two column types in the same expression disagree: {a_ty:?} vs {b_ty:?}"
            );
            a.clone()
        }
        (Ty::Column(_), Ty::Inferred(_)) => a.clone(),
        (Ty::Inferred(_), Ty::Column(_)) => b.clone(),
        (Ty::Inferred(a_ty), Ty::Inferred(b_ty)) => {
            assert_eq!(
                a_ty, b_ty,
                "two inferred types in the same expression disagree: {a_ty:?} vs {b_ty:?}"
            );
            a.clone()
        }

        // Records — merge field-by-field
        (Ty::Record(a_fields), Ty::Record(b_fields)) if a_fields.len() == b_fields.len() => {
            Ty::Record(
                a_fields
                    .iter()
                    .zip(b_fields)
                    .map(|(a, b)| merge(a, b))
                    .collect(),
            )
        }

        // Lists — merge element types
        (Ty::List(a_elem), Ty::List(b_elem)) => Ty::List(Box::new(merge(a_elem, b_elem))),

        _ => panic!("cannot merge incompatible types: {a:?} and {b:?}"),
    }
}

// ============================================================================
// Helpers
// ============================================================================

#[cfg(test)]
mod tests;
