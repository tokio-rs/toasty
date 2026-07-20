//! Phases 2 & 3: bidirectional type inference. A single recursive walk
//! synthesizes each node's type bottom-up and checks refined types back down
//! into `Arg` params, so each param ends up with a precise `db::Type`.

use toasty_core::{schema::db, stmt};

use super::{Param, Ty};

/// Expression context bound to the database schema.
type Cx<'a> = stmt::ExprContext<'a, db::Schema>;

/// Refine param types by walking the statement with synthesize + check.
pub(super) fn refine_param_types(
    stmt: &stmt::Statement,
    db_schema: &db::Schema,
    params: &mut [Param],
) {
    let cx = stmt::ExprContext::new(db_schema);
    refine_stmt(stmt, &cx, db_schema, params);
}

fn refine_stmt(stmt: &stmt::Statement, cx: &Cx<'_>, db_schema: &db::Schema, params: &mut [Param]) {
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

/// Lift a column's `db::Type` into the inferred-type form. List columns
/// expand to `Ty::List(Ty::Column(elem))` so they unify with values whose
/// inferred shape is also `Ty::List(_)`; everything else stays as a flat
/// `Ty::Column(_)`.
///
/// # Why this shape instead of widening `Ty::Column` to hold a `db::Type::List`?
///
/// `Ty` exists to carry *provenance* alongside the inferred type:
/// `Ty::Column(_)` is authoritative (from the schema), `Ty::Inferred(_)` is a
/// guess from a value. Merging the two is what propagates the column type
/// down into argument placeholders.
///
/// A list arg comes in as `Ty::List(Ty::Inferred(elem))` because the
/// element type is guessed from the first non-null value. When the schema
/// knows the column type, we need to merge the column-provenance element type
/// *into* the list. That requires the two sides to agree on shape —
/// `Ty::List(_)` vs `Ty::List(_)` — and merge element-wise via the list branch
/// in [`merge`].
///
/// The alternative of carrying `Ty::Column(db::Type::List(_))` would put a
/// list inside a "scalar" variant; merging it against `Ty::List(Inferred(_))`
/// from a value would either require a special case or lose the element-level
/// provenance the synthesize/check pass relies on. Expanding into
/// `Ty::List(Ty::Column(_))` keeps the data structure uniform — every list
/// is `Ty::List`, every scalar is `Ty::Column`/`Ty::Inferred` — and lets
/// `merge` handle the cases with no extra branches.
fn ty_from_column(storage_ty: db::Type) -> Ty {
    match storage_ty {
        db::Type::List(elem) => Ty::List(Box::new(ty_from_column(*elem))),
        scalar => Ty::Column(scalar),
    }
}

fn refine_insert(
    insert: &stmt::Insert,
    _cx: &Cx<'_>,
    db_schema: &db::Schema,
    params: &mut [Param],
) {
    let stmt::InsertTarget::Table(table) = &insert.target else {
        return;
    };
    let db_table = &db_schema.tables[table.table.0];

    match &insert.source.body {
        stmt::ExprSet::Values(values) => {
            // The column list supplies the authoritative type for each field.
            // This also maps document values to their scalar storage type.
            let expected = Ty::Record(
                table
                    .columns
                    .iter()
                    .map(|col_id| ty_from_column(db_table.columns[col_id.index].storage_ty.clone()))
                    .collect(),
            );

            for row in &values.rows {
                check(row, &expected, params);
            }
        }
        stmt::ExprSet::Select(select) => refine_source(&select.source, params),
        _ => {}
    }

    if let Some(upsert) = &insert.upsert {
        refine_assignments(&upsert.shared, db_table, params);
        refine_assignments(&upsert.defaults, db_table, params);
        refine_assignments(&upsert.update_defaults, db_table, params);
    }
}

fn refine_update(update: &stmt::Update, cx: &Cx<'_>, db_schema: &db::Schema, params: &mut [Param]) {
    // Refine assignment types from target columns
    if let stmt::UpdateTarget::Table(table_id) = &update.target {
        let db_table = &db_schema.tables[table_id.0];

        refine_assignments(&update.assignments, db_table, params);
    }

    // Refine filter types
    refine_filter(&update.filter, cx, params);
}

fn refine_assignments(assignments: &stmt::Assignments, db_table: &db::Table, params: &mut [Param]) {
    for (projection, assignment) in assignments.iter() {
        let steps = projection.as_slice();
        assert_eq!(
            steps.len(),
            1,
            "UPDATE assignment projection should be a single column index, got {steps:?}"
        );
        let col_idx = steps[0];
        let Some(col) = db_table.columns.get(col_idx) else {
            continue;
        };

        match assignment {
            stmt::Assignment::Set(expr) | stmt::Assignment::Append(expr) => {
                // The expression takes the column's full type (the whole
                // column for `Set`, the elements for `Append`). A
                // `#[document]` column's `Value::List`/`Value::Object` param
                // collapses to its scalar `Document` storage type via `merge`.
                let expected = ty_from_column(col.storage_ty.clone());
                check(expr, &expected, params);
            }
            // `Remove` is `array_remove(col, $1)`-shaped: the rhs binds
            // as the column's element type, not the list type. Pull the
            // element out of the list column type so the param is bound
            // correctly.
            stmt::Assignment::Remove(expr) => {
                if let db::Type::List(elem) = &col.storage_ty {
                    let expected = ty_from_column((**elem).clone());
                    check(expr, &expected, params);
                }
            }
            // `RemoveAt` binds an integer index, not a column value:
            // the column's element type is unrelated to the index's
            // type. Skip the column-driven `check` — the value-side
            // inference from `infer_ty` (e.g.
            // `Ty::Inferred(UnsignedInteger(8))` for a `usize`
            // converted to `Value::U64`) is enough to bind the param.
            stmt::Assignment::RemoveAt(_) | stmt::Assignment::Pop => {}
            // `Add` / `Subtract` bind a scalar of the column's type
            // (`col = col + $1`).
            stmt::Assignment::Add(expr) | stmt::Assignment::Subtract(expr) => {
                let expected = ty_from_column(col.storage_ty.clone());
                check(expr, &expected, params);
            }
            stmt::Assignment::Insert(_) | stmt::Assignment::Batch(_) => continue,
        }
    }
}

fn refine_query(query: &stmt::Query, cx: &Cx<'_>, params: &mut [Param]) {
    // One scope per query — matching the `ExprColumn::nesting` model and the
    // SQL serializer (which also scopes once per `Query`). `Query`'s target
    // resolves through its body to the `Select` source, so this single scope
    // is the source scope. Scoping the `Select` again would double-count a
    // level, so a column inside a subquery that references an outer column
    // (e.g. a JOIN-include's linking column lifted into an `EXISTS`) would
    // resolve against the wrong source.
    let cx = cx.scope(query);

    match &query.body {
        stmt::ExprSet::Select(select) => {
            refine_source(&select.source, params);
            refine_filter(&select.filter, &cx, params);
        }
        stmt::ExprSet::Values(values) => {
            for row in &values.rows {
                synthesize(row, &cx, params);
            }
        }
        // Data-modifying CTE bodies (a conditional write compiled to a CTE):
        // the write's assignments and filter carry params that need the same
        // column-driven refinement as a top-level UPDATE/DELETE.
        stmt::ExprSet::Update(update) => {
            refine_update(update, &cx, cx.schema(), params);
        }
        stmt::ExprSet::Delete(delete) => {
            refine_filter(&delete.filter, &cx, params);
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

fn refine_source(source: &stmt::Source, params: &mut [Param]) {
    let stmt::Source::Table(source) = source else {
        return;
    };

    for table in &source.tables {
        let stmt::TableRef::Func(stmt::ExprFunc::Unnest(unnest)) = table else {
            continue;
        };

        // The function stores each element type with its argument. Applying
        // the corresponding array type resolves all-NULL array parameters.
        for arg in &unnest.args {
            let array_ty = db::Type::list(arg.elem_ty.clone());
            check(&arg.expr, &ty_from_column(array_ty), params);
        }
    }
}

fn refine_filter(filter: &stmt::Filter, cx: &Cx<'_>, params: &mut [Param]) {
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
pub(super) fn synthesize(expr: &stmt::Expr, cx: &Cx<'_>, params: &mut [Param]) -> Ty {
    match expr {
        // Arg — type comes from the extracted param (whatever the current
        // inference state is — `Inferred(...)` from the value, possibly
        // already upgraded to `Column(...)` by a prior `check`).
        stmt::Expr::Arg(arg) => params[arg.position].ty.clone(),

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

        // AnyOp / AllOp — synthesize lhs, then push `List(lhs_ty)` down so
        // the rhs Arg's param type lifts to `db::Type::List(<elem>)` with
        // the column-known element type.
        stmt::Expr::AnyOp(e) => {
            let lhs_ty = synthesize(&e.lhs, cx, params);
            check(&e.rhs, &Ty::List(Box::new(lhs_ty)), params);
            Ty::Inferred(db::Type::Boolean)
        }
        stmt::Expr::AllOp(e) => {
            let lhs_ty = synthesize(&e.lhs, cx, params);
            check(&e.rhs, &Ty::List(Box::new(lhs_ty)), params);
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
/// merge the expected type into `params[n].ty` so column provenance and
/// concrete element types propagate down (e.g. `List(Unknown) → List(Column(_))`).
fn check(expr: &stmt::Expr, expected: &Ty, params: &mut [Param]) {
    match (expr, expected) {
        // Arg — merge expected into the param's current type. `merge` handles
        // provenance (column wins over inferred) and unknowns (any type wins
        // over Unknown), including recursively for list element types.
        (stmt::Expr::Arg(arg), ty) => {
            let current = params[arg.position].ty.clone();
            params[arg.position].ty = merge(&current, ty);
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
fn check_list(list_expr: &stmt::Expr, elem_ty: &Ty, params: &mut [Param]) {
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
pub(super) fn merge(a: &Ty, b: &Ty) -> Ty {
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

        // A `#[document]` collection binds as a `Value::List` (the JSON array's
        // internal shape) while its column storage type is the opaque scalar
        // `Document`. The schema type is authoritative, so the list resolves to
        // the document column type — the value keeps its list shape, only its
        // type resolves. This lets `check` type a document column like any other.
        (Ty::List(_), col @ Ty::Column(db::Type::Document { .. }))
        | (col @ Ty::Column(db::Type::Document { .. }), Ty::List(_)) => col.clone(),

        _ => panic!("cannot merge incompatible types: {a:?} and {b:?}"),
    }
}
