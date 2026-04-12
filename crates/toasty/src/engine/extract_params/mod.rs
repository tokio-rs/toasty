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
/// Unlike `db::Type` which is scalar-only, this supports structured types
/// (records, lists) so that type information can flow through composite
/// expressions like `($value, $column) == ($column, $value)`.
#[derive(Debug, Clone)]
enum InferredType {
    /// A concrete scalar storage type.
    Scalar(db::Type),
    /// A tuple of types (one per field).
    Record(Vec<InferredType>),
    /// A homogeneous list where all elements share a type.
    List(Box<InferredType>),
    /// Type could not be determined.
    Unknown,
}

// ============================================================================
// Phase 1: Mechanical value extraction
// ============================================================================

/// Replace all scalar `Value` nodes with `Arg(n)` placeholders.
/// Initialize each param's `ty` from the value itself.
fn extract_values(stmt: &mut stmt::Statement, params: &mut Vec<TypedValue>) {
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

fn is_extractable_scalar(value: &stmt::Value) -> bool {
    !matches!(
        value,
        stmt::Value::Null | stmt::Value::Record(_) | stmt::Value::List(_)
    )
}

// ============================================================================
// Phase 2+3: Bidirectional type inference
// ============================================================================

/// Refine param types by walking the statement with synthesize + check.
fn refine_param_types(stmt: &stmt::Statement, db_schema: &db::Schema, params: &mut [TypedValue]) {
    let cx = stmt::ExprContext::new(db_schema);

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
            refine_query(query, &cx, params);
        }
    }
}

fn refine_insert(
    insert: &stmt::Insert,
    _cx: &Cx<'_>,
    db_schema: &db::Schema,
    params: &mut [TypedValue],
) {
    // Build expected type from column list
    let expected = match &insert.target {
        stmt::InsertTarget::Table(table) => {
            let db_table = &db_schema.tables[table.table.0];
            let field_types: Vec<InferredType> = table
                .columns
                .iter()
                .map(|col_id| {
                    InferredType::Scalar(db_table.columns[col_id.index].storage_ty.clone())
                })
                .collect();
            InferredType::Record(field_types)
        }
        _ => InferredType::Unknown,
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
                if let Some(&col_idx) = steps.first()
                    && let Some(col) = db_table.columns.get(col_idx)
                {
                    let expected = InferredType::Scalar(col.storage_ty.clone());
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
            // Subquery VALUES (e.g., derived tables) — synthesize each row
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
        // Synthesize triggers check internally for BinaryOp, InList, etc.
        synthesize(expr, cx, params);
    }
}

// ============================================================================
// Synthesize (bottom-up) — returns the inferred type
// ============================================================================

/// Compute the inferred type of an expression from its children.
///
/// For comparison operators, this also triggers `check()` to push refined
/// types down into both sides (bidirectional inference).
fn synthesize(expr: &stmt::Expr, cx: &Cx<'_>, params: &mut [TypedValue]) -> InferredType {
    match expr {
        // Arg — type comes from the extracted param
        stmt::Expr::Arg(arg) => {
            if let Some(tv) = params.get(arg.position) {
                InferredType::Scalar(tv.ty.clone())
            } else {
                InferredType::Unknown
            }
        }

        // Column reference — resolve from schema
        stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Column(_)) => {
            match cx.resolve_expr_reference(expr_ref) {
                stmt::ResolvedRef::Column(col) => InferredType::Scalar(col.storage_ty.clone()),
                _ => InferredType::Unknown,
            }
        }

        // Projection — type of the projected field
        stmt::Expr::Project(project) => {
            let base_ty = synthesize(&project.base, cx, params);
            match base_ty {
                InferredType::Record(fields) => {
                    let idx = project.projection.as_slice();
                    if let Some(&step) = idx.first() {
                        fields
                            .into_iter()
                            .nth(step)
                            .unwrap_or(InferredType::Unknown)
                    } else {
                        InferredType::Unknown
                    }
                }
                // Projecting from a scalar (e.g., discriminant from enum column)
                // — the result has the same type as the base
                ty => ty,
            }
        }

        // Record — synthesize each field
        stmt::Expr::Record(record) => {
            let fields: Vec<InferredType> = record
                .fields
                .iter()
                .map(|f| synthesize(f, cx, params))
                .collect();
            InferredType::Record(fields)
        }

        // List — synthesize each item, merge to a common type
        stmt::Expr::List(list) => {
            let mut merged = InferredType::Unknown;
            for item in &list.items {
                let item_ty = synthesize(item, cx, params);
                merged = merge(&merged, &item_ty);
            }
            InferredType::List(Box::new(merged))
        }

        // BinaryOp (comparison) — synthesize both sides, merge, check both
        stmt::Expr::BinaryOp(binary) => {
            let lhs_ty = synthesize(&binary.lhs, cx, params);
            let rhs_ty = synthesize(&binary.rhs, cx, params);
            let merged = merge(&lhs_ty, &rhs_ty);
            check(&binary.lhs, &merged, params);
            check(&binary.rhs, &merged, params);
            InferredType::Scalar(db::Type::Boolean)
        }

        // InList — synthesize expr, check list items against it
        stmt::Expr::InList(in_list) => {
            let expr_ty = synthesize(&in_list.expr, cx, params);
            synthesize(&in_list.list, cx, params);
            // Check each list item against the expression's type
            check_list(&in_list.list, &expr_ty, params);
            InferredType::Scalar(db::Type::Boolean)
        }

        // InSubquery — synthesize the expression, recurse into subquery
        stmt::Expr::InSubquery(in_sub) => {
            synthesize(&in_sub.expr, cx, params);
            refine_query(&in_sub.query, cx, params);
            InferredType::Scalar(db::Type::Boolean)
        }

        // Exists — recurse into subquery
        stmt::Expr::Exists(exists) => {
            refine_query(&exists.subquery, cx, params);
            InferredType::Scalar(db::Type::Boolean)
        }

        // Nested statement — we can't recurse with refine_param_types here
        // because we don't have the db::Schema directly. Nested Stmt expressions
        // are relatively rare; the values inside were already extracted in phase 1.
        stmt::Expr::Stmt(_) => InferredType::Unknown,

        // Logical operators — recurse, return boolean
        stmt::Expr::And(and) => {
            for op in &and.operands {
                synthesize(op, cx, params);
            }
            InferredType::Scalar(db::Type::Boolean)
        }
        stmt::Expr::Or(or) => {
            for op in &or.operands {
                synthesize(op, cx, params);
            }
            InferredType::Scalar(db::Type::Boolean)
        }
        stmt::Expr::Not(not) => {
            synthesize(&not.expr, cx, params);
            InferredType::Scalar(db::Type::Boolean)
        }
        stmt::Expr::IsNull(is_null) => {
            synthesize(&is_null.expr, cx, params);
            InferredType::Scalar(db::Type::Boolean)
        }

        // Values that weren't extracted (Null, Default)
        stmt::Expr::Value(stmt::Value::Null) => InferredType::Unknown,
        stmt::Expr::Default => InferredType::Unknown,

        // Anything else
        _ => InferredType::Unknown,
    }
}

// ============================================================================
// Check (top-down) — pushes refined types into Arg nodes
// ============================================================================

/// Push an expected type down into an expression. When it reaches `Arg(n)`,
/// refine `params[n].ty` if the expected type is more specific.
fn check(expr: &stmt::Expr, expected: &InferredType, params: &mut [TypedValue]) {
    match (expr, expected) {
        // Arg — refine the param's type
        (stmt::Expr::Arg(arg), InferredType::Scalar(expected_ty)) => {
            if let Some(tv) = params.get_mut(arg.position) {
                tv.ty = more_specific(&tv.ty, expected_ty);
            }
        }

        // Record — check each field against its expected type
        (stmt::Expr::Record(record), InferredType::Record(field_types)) => {
            for (field, field_ty) in record.fields.iter().zip(field_types) {
                check(field, field_ty, params);
            }
        }

        // List — check each item against the expected element type
        (stmt::Expr::List(list), InferredType::List(elem_ty)) => {
            for item in &list.items {
                check(item, elem_ty, params);
            }
        }
        (stmt::Expr::List(list), ty @ InferredType::Scalar(_)) => {
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
fn check_list(list_expr: &stmt::Expr, elem_ty: &InferredType, params: &mut [TypedValue]) {
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

/// Merge two inferred types, picking the more specific one at each position.
fn merge(a: &InferredType, b: &InferredType) -> InferredType {
    match (a, b) {
        (InferredType::Unknown, other) | (other, InferredType::Unknown) => other.clone(),
        (InferredType::Scalar(a), InferredType::Scalar(b)) => {
            InferredType::Scalar(more_specific(a, b))
        }
        (InferredType::Record(a), InferredType::Record(b)) if a.len() == b.len() => {
            InferredType::Record(a.iter().zip(b).map(|(a, b)| merge(a, b)).collect())
        }
        (InferredType::List(a), InferredType::List(b)) => InferredType::List(Box::new(merge(a, b))),
        // Incompatible structures — keep the first
        _ => a.clone(),
    }
}

/// Pick the more specific of two scalar db::Types.
///
/// A type is "more specific" if it carries additional information beyond what
/// the value's natural type provides. For example, `Enum(..)` is more specific
/// than `Text` because the value is a string but the column needs the enum OID.
fn more_specific(a: &db::Type, b: &db::Type) -> db::Type {
    match (a, b) {
        // Enum is more specific than Text (enum values are strings)
        (db::Type::Enum(_), db::Type::Text) => a.clone(),
        (db::Type::Text, db::Type::Enum(_)) => b.clone(),
        // VarChar is more specific than Text
        (db::Type::VarChar(_), db::Type::Text) => a.clone(),
        (db::Type::Text, db::Type::VarChar(_)) => b.clone(),
        // Otherwise keep the first (they should be compatible)
        _ => a.clone(),
    }
}

// ============================================================================
// Helpers
// ============================================================================

#[cfg(test)]
mod tests;
