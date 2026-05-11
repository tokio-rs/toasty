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
    driver::{Capability, operation::TypedValue},
    schema::{Schema, db},
    stmt::{self, VisitMut},
};

/// Expression context bound to the database schema.
type Cx<'a> = stmt::ExprContext<'a, db::Schema>;

// ============================================================================
// Public entry point
// ============================================================================

/// Extract bind parameters from a statement, replacing scalar values with
/// `Expr::Arg(n)` placeholders and inferring precise `db::Type` for each.
pub(crate) fn extract_params(
    stmt: &mut stmt::Statement,
    schema: &Schema,
    capability: &Capability,
) -> Vec<TypedValue> {
    // Phase 1: Mechanical extraction — replace values with Arg(n)
    let mut params: Vec<Param> = Vec::new();
    extract_values(stmt, &mut params, capability);

    // Phase 2+3: Bidirectional type inference — refine param types
    refine_param_types(stmt, &schema.db, &mut params);

    // Materialize the final TypedValues. `finalize_ty` panics if any param
    // is still unresolved — synthesize/check is expected to type every param.
    params
        .into_iter()
        .map(|p| {
            let Param { value, ty } = p;
            TypedValue {
                ty: finalize_ty(&value, ty),
                value,
            }
        })
        .collect()
}

/// A bind parameter being inferred. Once inference completes, the `Ty` is
/// converted to a concrete `db::Type` for the `TypedValue`.
struct Param {
    value: stmt::Value,
    ty: Ty,
}

/// Resolve a `Ty` to a concrete `db::Type`. Panics on `Unknown` / `Record` —
/// every param should be fully inferred by the synthesize/check pass; if a
/// statement reaches here with an unresolved param, that's a bug worth
/// surfacing so we can evaluate the specific case.
fn finalize_ty(value: &stmt::Value, ty: Ty) -> db::Type {
    match ty {
        Ty::Column(t) | Ty::Inferred(t) => t,
        Ty::List(elem) => db::Type::List(Box::new(finalize_ty(value, *elem))),
        Ty::Unknown => panic!("extract_params left {value:?} with unresolved type"),
        Ty::Record(_) => panic!(
            "extract_params left {value:?} typed as a record; only scalars and lists are extracted as params"
        ),
    }
}

/// Initial type guess for a value, used as the starting point for inference.
///
/// Returns the most precise `Ty` derivable from the value alone:
/// - Scalars become `Ty::Inferred(<db::Type>)`.
/// - Lists become `Ty::List(<elem>)`, recursing into the first non-null item.
///   Empty / all-null lists yield `Ty::List(Ty::Unknown)`; the element type is
///   refined by synthesize/check when a column context is available.
/// - Anything we can't classify (`Null`, `Record`, `F32`/`F64`, `Zoned`,
///   `BigDecimal`, `SparseRecord`) becomes `Ty::Unknown`.
fn infer_ty(value: &stmt::Value) -> Ty {
    use stmt::Value;
    match value {
        Value::Bool(_) => Ty::Inferred(db::Type::Boolean),
        Value::I8(_) => Ty::Inferred(db::Type::Integer(1)),
        Value::I16(_) => Ty::Inferred(db::Type::Integer(2)),
        Value::I32(_) => Ty::Inferred(db::Type::Integer(4)),
        Value::I64(_) => Ty::Inferred(db::Type::Integer(8)),
        Value::U8(_) => Ty::Inferred(db::Type::UnsignedInteger(1)),
        Value::U16(_) => Ty::Inferred(db::Type::UnsignedInteger(2)),
        Value::U32(_) => Ty::Inferred(db::Type::UnsignedInteger(4)),
        Value::U64(_) => Ty::Inferred(db::Type::UnsignedInteger(8)),
        Value::String(_) => Ty::Inferred(db::Type::Text),
        Value::Uuid(_) => Ty::Inferred(db::Type::Uuid),
        Value::Bytes(_) => Ty::Inferred(db::Type::Blob),
        #[cfg(feature = "rust_decimal")]
        Value::Decimal(_) => Ty::Inferred(db::Type::Numeric(None)),
        #[cfg(feature = "jiff")]
        Value::Timestamp(_) => Ty::Inferred(db::Type::Timestamp(6)),
        #[cfg(feature = "jiff")]
        Value::Date(_) => Ty::Inferred(db::Type::Date),
        #[cfg(feature = "jiff")]
        Value::Time(_) => Ty::Inferred(db::Type::Time(6)),
        #[cfg(feature = "jiff")]
        Value::DateTime(_) => Ty::Inferred(db::Type::DateTime(6)),
        Value::List(items) => {
            let elem = items
                .iter()
                .find(|v| !v.is_null())
                .map(infer_ty)
                .unwrap_or(Ty::Unknown);
            Ty::List(Box::new(elem))
        }
        _ => Ty::Unknown,
    }
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
    #[cfg(test)]
    fn is_column(&self) -> bool {
        matches!(self, Ty::Column(_))
    }
}

// ============================================================================
// Phase 1: Mechanical value extraction
// ============================================================================

/// Replace all scalar `Value` nodes with `Arg(n)` placeholders.
/// Initialize each param's `ty` from the value itself.
fn extract_values(stmt: &mut stmt::Statement, params: &mut Vec<Param>, capability: &Capability) {
    struct Extract<'a> {
        params: &'a mut Vec<Param>,
        bind_list_param: bool,
    }

    impl stmt::VisitMut for Extract<'_> {
        fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
            // Intercept ANY/ALL: bind their array operand as one Value::List
            // param rather than visiting the rhs and extracting each element
            // separately. The element type is refined to the column type by
            // the synthesize/check pass.
            match expr {
                stmt::Expr::AnyOp(e) => {
                    self.visit_expr_mut(&mut e.lhs);
                    if let Some(arg) = extract_array_operand(&mut e.rhs, self.params) {
                        *e.rhs = arg;
                    } else {
                        self.visit_expr_mut(&mut e.rhs);
                    }
                    return;
                }
                stmt::Expr::AllOp(e) => {
                    self.visit_expr_mut(&mut e.lhs);
                    if let Some(arg) = extract_array_operand(&mut e.rhs, self.params) {
                        *e.rhs = arg;
                    } else {
                        self.visit_expr_mut(&mut e.rhs);
                    }
                    return;
                }
                _ => {}
            }

            // On backends that bind arrays as a single protocol parameter
            // (PostgreSQL, see `Capability::bind_list_param`), a literal
            // list of scalar values is the value of a `Vec<scalar>` model
            // field — extract as one `Value::List` arg so it round-trips
            // through the driver as a `text[]` / `int8[]` bind. Without
            // this, recursion would expand the list to one arg per item
            // and render it as a SQL record literal.
            if self.bind_list_param
                && is_scalar_list(expr)
                && let Some(arg) = extract_array_operand(expr, self.params)
            {
                *expr = arg;
                return;
            }

            // Default post-order: recurse first, then maybe extract this node.
            stmt::visit_mut::visit_expr_mut(self, expr);

            match expr {
                stmt::Expr::Value(value) if is_extractable_scalar(value) => {
                    let ty = infer_ty(value);
                    let position = self.params.len();
                    let value = std::mem::replace(value, stmt::Value::Null);
                    self.params.push(Param { value, ty });
                    *expr = stmt::Expr::arg(position);
                }
                stmt::Expr::Value(value @ (stmt::Value::Record(_) | stmt::Value::List(_))) => {
                    let owned = std::mem::replace(value, stmt::Value::Null);
                    *expr = value_to_extracted_expr(owned, self.params, self.bind_list_param);
                }
                _ => {}
            }
        }
    }

    Extract {
        params,
        bind_list_param: capability.bind_list_param,
    }
    .visit_mut(stmt);
}

/// Whether `expr` is an `Expr::Value` carrying an extractable scalar.
fn is_extractable_scalar_expr(expr: &stmt::Expr) -> bool {
    matches!(expr, stmt::Expr::Value(v) if is_extractable_scalar(v))
}

/// Whether `expr` is a literal list of scalar values — either an
/// `Expr::List` of `Expr::Value(...)` items, or an already-collapsed
/// `Expr::Value(Value::List(...))`. The canonicalizer (`fold::expr_list`)
/// produces the latter shape, but lowering can still emit the former, so
/// we cover both.
fn is_scalar_list(expr: &stmt::Expr) -> bool {
    match expr {
        stmt::Expr::List(list) => list.items.iter().all(is_extractable_scalar_expr),
        stmt::Expr::Value(stmt::Value::List(items)) => items.iter().all(is_extractable_scalar),
        _ => false,
    }
}

/// If `expr` is a list literal of values, take it out, push one
/// `Param { value: Value::List(items), ty: Ty::List(<elem>) }` onto `params`,
/// and return an `Expr::Arg(n)` to put back in its place. Used for both the
/// `ANY/ALL` rhs operand and `Vec<scalar>` field literals on backends that
/// bind arrays as a single protocol parameter.
///
/// The element type starts as the value-inferred type of the first non-null
/// item — or `Ty::Unknown` for empty / all-null lists. The synthesize/check
/// pass refines it to the column type when one is known.
fn extract_array_operand(expr: &mut stmt::Expr, params: &mut Vec<Param>) -> Option<stmt::Expr> {
    let items: Vec<stmt::Value> = match expr {
        stmt::Expr::Value(stmt::Value::List(_)) => {
            let stmt::Expr::Value(stmt::Value::List(items)) =
                std::mem::replace(expr, stmt::Expr::null())
            else {
                unreachable!()
            };
            items
        }
        stmt::Expr::List(list) if list.items.iter().all(|i| matches!(i, stmt::Expr::Value(_))) => {
            let stmt::Expr::List(list) = std::mem::replace(expr, stmt::Expr::null()) else {
                unreachable!()
            };
            list.items
                .into_iter()
                .map(|e| match e {
                    stmt::Expr::Value(v) => v,
                    _ => unreachable!(),
                })
                .collect()
        }
        _ => return None,
    };

    let value = stmt::Value::List(items);
    let ty = infer_ty(&value);

    let position = params.len();
    params.push(Param { value, ty });
    Some(stmt::Expr::arg(position))
}

/// Recursively convert a `Value` into an `Expr`, extracting scalar values.
/// Takes ownership to avoid cloning.
///
/// On backends that bind arrays as a single protocol parameter (`bind_list_param`),
/// a `Value::List` of all extractable scalars is captured as a single param of
/// `Value::List` shape so it round-trips through the driver as one array bind.
/// Other lists fall through to per-element expansion to preserve the existing
/// record/tuple semantics on backends without native array binds.
fn value_to_extracted_expr(
    value: stmt::Value,
    params: &mut Vec<Param>,
    bind_list_param: bool,
) -> stmt::Expr {
    match value {
        stmt::Value::Null => stmt::Expr::Value(stmt::Value::Null),
        stmt::Value::Record(record) => {
            let fields = record
                .fields
                .into_iter()
                .map(|f| value_to_extracted_expr(f, params, bind_list_param))
                .collect();
            stmt::Expr::Record(stmt::ExprRecord::from_vec(fields))
        }
        stmt::Value::List(values)
            if bind_list_param && values.iter().all(is_extractable_scalar) =>
        {
            let value = stmt::Value::List(values);
            let ty = infer_ty(&value);
            let position = params.len();
            params.push(Param { value, ty });
            stmt::Expr::arg(position)
        }
        stmt::Value::List(values) => {
            let items = values
                .into_iter()
                .map(|v| value_to_extracted_expr(v, params, bind_list_param))
                .collect();
            stmt::Expr::List(stmt::ExprList { items })
        }
        scalar => {
            let ty = infer_ty(&scalar);
            let position = params.len();
            params.push(Param { value: scalar, ty });
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
fn refine_param_types(stmt: &stmt::Statement, db_schema: &db::Schema, params: &mut [Param]) {
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
/// element type is guessed from the first non-null value (see
/// [`infer_ty`]). When the schema knows the column type, we need to merge
/// the column-provenance element type *into* the list. That requires the
/// two sides to agree on shape — `Ty::List(_)` vs `Ty::List(_)` — and merge
/// element-wise via the existing list branch in [`merge`].
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
    // Build expected type from column list (authoritative)
    let expected = match &insert.target {
        stmt::InsertTarget::Table(table) => {
            let db_table = &db_schema.tables[table.table.0];
            let field_types: Vec<Ty> = table
                .columns
                .iter()
                .map(|col_id| ty_from_column(db_table.columns[col_id.index].storage_ty.clone()))
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

fn refine_update(update: &stmt::Update, cx: &Cx<'_>, db_schema: &db::Schema, params: &mut [Param]) {
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
                    let expected = ty_from_column(col.storage_ty.clone());
                    check(expr, &expected, params);
                }
            }
        }
    }

    // Refine filter types
    refine_filter(&update.filter, cx, params);
}

fn refine_query(query: &stmt::Query, cx: &Cx<'_>, params: &mut [Param]) {
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
fn synthesize(expr: &stmt::Expr, cx: &Cx<'_>, params: &mut [Param]) -> Ty {
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
