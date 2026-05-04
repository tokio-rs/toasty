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
    stmt::{self, visit_mut::VisitMut},
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
    let mut params = Vec::new();
    let mut extractor = Extractor {
        params: &mut params,
        capability,
        decompose_depth: 0,
    };
    extractor.visit_stmt_mut(stmt);

    // Phase 1.5: For backends with `in_array` support, phase 1 bundled
    // every IN-list rhs whose elements were scalars. Some lhs column
    // types (notably native enums) have no driver-level array path; for
    // those we decompose the bundled Arg back into per-element Args.
    if capability.in_array {
        let cx = stmt::ExprContext::new(&schema.db);
        decompose_unsupported_in_lists(stmt, &cx, &mut params);
    }

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

/// Stateful AST visitor that replaces `Value` nodes with `Arg(n)`
/// placeholders.
///
/// Whether a `Value::List` of scalars rides as one parameter or decomposes
/// into per-element parameters depends on two backend capabilities and on
/// **position**, tracked through the visit:
///
/// - [`Capability::array_binding`] — driver can bind a `Value::List` at
///   all. Without it, no list ever bundles.
/// - [`Capability::in_array`] — `IN <bound_array>` is a valid SQL form.
///   Without it, the rhs of an `IN` predicate must decompose to
///   `(?1, ?2, …)` even when the driver could bind an array elsewhere.
///
/// For an `IN`-list rhs, the visitor additionally checks whether the lhs
/// column's storage type is one the driver can bind as an array. If not
/// (e.g. a native `Enum`), the rhs decomposes regardless — `array_binding`
/// alone isn't enough when the array element type has no driver-level
/// array path.
///
/// `decompose_depth` counts enclosing positions where bundling is *forbidden*
/// by capability alone — namely an `IN`-list rhs on a backend without
/// `in_array`. Zero means bundling is fine.
struct Extractor<'a> {
    params: &'a mut Vec<TypedValue>,
    capability: &'a Capability,
    decompose_depth: u32,
}

impl Extractor<'_> {
    /// Whether a `Value::List` of scalars at the current position should be
    /// kept whole (one bind parameter) rather than decomposed into per-
    /// element parameters.
    fn can_bundle_scalar_list(&self) -> bool {
        self.capability.array_binding && self.decompose_depth == 0
    }

    /// Push `value` as a fresh parameter and return the matching `Arg`.
    fn push_param(&mut self, value: stmt::Value) -> stmt::Expr {
        let ty = db::Type::from_value(&value);
        let position = self.params.len();
        self.params.push(TypedValue { value, ty });
        stmt::Expr::arg(position)
    }

    /// Recursively destructure a non-extractable composite value. Used for
    /// rows in batch INSERT (`Value::Record` of fields) and for IN-list-rhs
    /// lists that the backend cannot accept whole.
    ///
    /// Field positions inside a destructured `Record` revisit the same
    /// "scalar-list = one parameter" decision: a `Vec<scalar>` field of an
    /// INSERT row should bundle even though its enclosing record had to
    /// destructure.
    fn destructure(&mut self, value: stmt::Value) -> stmt::Expr {
        match value {
            stmt::Value::Null => stmt::Expr::Value(stmt::Value::Null),
            stmt::Value::Record(record) => {
                let fields = record
                    .fields
                    .into_iter()
                    .map(|f| self.destructure_field(f))
                    .collect();
                stmt::Expr::Record(stmt::ExprRecord::from_vec(fields))
            }
            stmt::Value::List(values) => {
                let items = values.into_iter().map(|v| self.destructure(v)).collect();
                stmt::Expr::List(stmt::ExprList { items })
            }
            scalar => self.push_param(scalar),
        }
    }

    /// Like [`destructure`], but a `Value::List` of scalars is kept whole
    /// (one bind parameter) when the current context allows it. Used for
    /// the immediate fields of a record so a `Vec<scalar>` column value
    /// rides as one parameter even though its enclosing row record must
    /// be destructured.
    fn destructure_field(&mut self, value: stmt::Value) -> stmt::Expr {
        if let stmt::Value::List(items) = &value
            && items.iter().all(is_scalar_leaf)
            && self.can_bundle_scalar_list()
        {
            return self.push_param(value);
        }
        self.destructure(value)
    }
}

impl VisitMut for Extractor<'_> {
    /// IN-list rhs gets a context flag pushed around just the `list` child.
    /// The lhs is visited normally — only the rhs participates in the
    /// bundle / decompose decision.
    fn visit_expr_in_list_mut(&mut self, node: &mut stmt::ExprInList) {
        self.visit_expr_mut(&mut node.expr);

        // Without `in_array`, IN-list rhs cannot be a bound array.
        let must_decompose = !self.capability.in_array;
        if must_decompose {
            self.decompose_depth += 1;
        }
        self.visit_expr_mut(&mut node.list);
        if must_decompose {
            self.decompose_depth -= 1;
        }
    }

    /// Post-order traversal. Children are processed first (scalar values
    /// inside them get extracted as their own parameters), then the
    /// current node decides whether it itself is a parameter, a structural
    /// expression, or untouched.
    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        stmt::visit_mut::visit_expr_mut(self, expr);

        match expr {
            // Plain scalar → one parameter.
            stmt::Expr::Value(value) if is_scalar_leaf(value) => {
                let value = std::mem::replace(value, stmt::Value::Null);
                *expr = self.push_param(value);
            }

            // Composite values (records, lists) need a context-aware decision.
            stmt::Expr::Value(value @ (stmt::Value::Record(_) | stmt::Value::List(_))) => {
                let owned = std::mem::replace(value, stmt::Value::Null);

                // A list of all-scalars in a position that supports an array
                // bind ships as one parameter. Anywhere else we destructure:
                //   - records always destructure (their fields are independent);
                //   - lists of records destructure (batch INSERT VALUES);
                //   - lists of scalars in an IN-list rhs without `in_list_array`
                //     destructure to per-element placeholders.
                if let stmt::Value::List(items) = &owned
                    && items.iter().all(is_scalar_leaf)
                    && self.can_bundle_scalar_list()
                {
                    *expr = self.push_param(owned);
                } else {
                    *expr = self.destructure(owned);
                }
            }

            _ => {}
        }
    }
}

/// A `Value` is a "scalar leaf" if it represents a single typed value that
/// the driver binds as one parameter — i.e. anything that isn't `Null`,
/// `Record`, or `List`.
fn is_scalar_leaf(value: &stmt::Value) -> bool {
    !matches!(
        value,
        stmt::Value::Null | stmt::Value::Record(_) | stmt::Value::List(_)
    )
}

// ============================================================================
// Phase 1.5: Decompose bundled IN-list rhs whose lhs has no array path
// ============================================================================

/// Returns true if a `db::Type` is one the driver array-binding path
/// supports across SQL backends. Excludes types whose array form needs
/// special driver handling — most importantly native enum types, where
/// PostgreSQL would need an OID-aware enum-array binder.
fn is_array_bindable_element(ty: &db::Type) -> bool {
    matches!(
        ty,
        db::Type::Boolean
            | db::Type::Integer(_)
            | db::Type::UnsignedInteger(_)
            | db::Type::Float(_)
            | db::Type::Text
            | db::Type::VarChar(_)
            | db::Type::Uuid
            | db::Type::Numeric(_)
    )
}

/// Walk the statement with proper scoping and undo the IN-list bundling
/// where the lhs's column type isn't one the driver can ship as an array.
/// The bundled rhs is `Expr::Arg(n)` whose param is `Value::List([...])`;
/// we replace the Arg with `Expr::List([Arg(m1), Arg(m2), …])` and the
/// single param with one per element. The original (now unreferenced)
/// param is replaced with `Value::Null` to avoid renumbering.
fn decompose_unsupported_in_lists(
    stmt: &mut stmt::Statement,
    cx: &Cx<'_>,
    params: &mut Vec<TypedValue>,
) {
    // The cx scoping borrows the statement immutably; the filter rewrite
    // needs it mutably. Take each filter out first (so the immutable
    // borrow that scoping needs is uncontested), do the rewrite, put it
    // back.
    match stmt {
        stmt::Statement::Insert(insert) => {
            // Take the filter out (releases the mut borrow on insert),
            // scope, rewrite, put back.
            let mut filter = match &mut insert.source.body {
                stmt::ExprSet::Select(select) => std::mem::take(&mut select.filter),
                _ => return,
            };
            {
                let cx_insert = cx.scope(&*insert);
                if let stmt::ExprSet::Select(select) = &insert.source.body {
                    let cx_select = cx_insert.scope(&**select);
                    decompose_in_filter(&mut filter, &cx_select, params);
                }
            }
            if let stmt::ExprSet::Select(select) = &mut insert.source.body {
                select.filter = filter;
            }
        }
        stmt::Statement::Update(update) => {
            let mut filter = std::mem::take(&mut update.filter);
            {
                let scoped = cx.scope(&*update);
                decompose_in_filter(&mut filter, &scoped, params);
            }
            update.filter = filter;
        }
        stmt::Statement::Delete(delete) => {
            let mut filter = std::mem::take(&mut delete.filter);
            {
                let scoped = cx.scope(&*delete);
                decompose_in_filter(&mut filter, &scoped, params);
            }
            delete.filter = filter;
        }
        stmt::Statement::Query(query) => {
            let mut filter = match &mut query.body {
                stmt::ExprSet::Select(select) => std::mem::take(&mut select.filter),
                _ => return,
            };
            {
                let cx_query = cx.scope(&*query);
                if let stmt::ExprSet::Select(select) = &query.body {
                    let cx_select = cx_query.scope(&**select);
                    decompose_in_filter(&mut filter, &cx_select, params);
                }
            }
            if let stmt::ExprSet::Select(select) = &mut query.body {
                select.filter = filter;
            }
        }
    }
}

fn decompose_in_filter(filter: &mut stmt::Filter, cx: &Cx<'_>, params: &mut Vec<TypedValue>) {
    if let Some(expr) = &mut filter.expr {
        decompose_in_expr(expr, cx, params);
    }
}

fn decompose_in_expr(expr: &mut stmt::Expr, cx: &Cx<'_>, params: &mut Vec<TypedValue>) {
    match expr {
        stmt::Expr::And(and) => {
            for op in &mut and.operands {
                decompose_in_expr(op, cx, params);
            }
        }
        stmt::Expr::Or(or) => {
            for op in &mut or.operands {
                decompose_in_expr(op, cx, params);
            }
        }
        stmt::Expr::Not(not) => decompose_in_expr(&mut not.expr, cx, params),
        stmt::Expr::InList(in_list) => {
            // Only act on the bundled-Arg shape — a decomposed `Expr::List`
            // rhs already has individual placeholders.
            let stmt::Expr::Arg(arg) = &*in_list.list else {
                return;
            };
            let arg_pos = arg.position;

            // Resolve lhs column type. Bail if lhs isn't a column ref or
            // resolves to something else (e.g. a foreign-key field that
            // lowering didn't simplify to a column).
            let stmt::Expr::Reference(reference @ stmt::ExprReference::Column(_)) = &*in_list.expr
            else {
                return;
            };
            let stmt::ResolvedRef::Column(col) = cx.resolve_expr_reference(reference) else {
                return;
            };

            if is_array_bindable_element(&col.storage_ty) {
                return;
            }

            // Pull the bundled list out of params, leave a Null sentinel
            // behind to keep param positions stable for any other
            // references (there shouldn't be any, but it's cheap insurance).
            let stmt::Value::List(items) =
                std::mem::replace(&mut params[arg_pos].value, stmt::Value::Null)
            else {
                // Not a bundled list — nothing to decompose.
                return;
            };
            params[arg_pos].ty = db::Type::from_value(&stmt::Value::Null);

            // Append one param per element, build the matching List of Args.
            let new_items: Vec<stmt::Expr> = items
                .into_iter()
                .map(|value| {
                    let ty = db::Type::from_value(&value);
                    let position = params.len();
                    params.push(TypedValue { value, ty });
                    stmt::Expr::arg(position)
                })
                .collect();

            *in_list.list = stmt::Expr::List(stmt::ExprList { items: new_items });
        }
        _ => {}
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
        // Bundled IN-list rhs: a single `Arg` whose param holds the whole
        // array. The param needs the array type (e.g., `db::Type::Array(Text)`),
        // not the scalar element type, so the driver binds it as an array.
        stmt::Expr::Arg(arg) if elem_ty.is_column() => {
            if let Some(elem_db_ty) = elem_ty.db_type() {
                params[arg.position].ty = db::Type::Array(Box::new(elem_db_ty.clone()));
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
