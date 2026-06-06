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
    stmt::{self, IntoExprTarget, VisitMut},
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
    // Phase 0a: lower document-path reads. A filter or returning expression
    // that projects into a `#[document]` column travels through the engine as a
    // plain `ExprProject` — identical to a column-expanded embed — so the
    // in-memory interpreter can evaluate it. Here, at the single choke point
    // every SQL statement passes through on its way to a driver, rewrite it to
    // the `FuncJsonExtract` the serializer renders. The driver only ever sees
    // the JSON node, never the projection.
    lower_document_paths(stmt, schema);

    // Phase 0b: rewrite bare `#[document]` embed values into the named
    // `Value::Object` form *before* generic extraction. The generic path
    // expands a `Value::Record` into a SQL row-value tuple `(?, ?)`; a document
    // column needs the whole embed bound as one document param instead.
    mark_document_values(stmt, &schema.db);

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

// ============================================================================
// Phase 0a: lower document-path reads to JSON extraction (the SQL edge)
// ============================================================================

/// Rewrite every projection into a `#[document]` column into the
/// [`FuncJsonExtract`](stmt::FuncJsonExtract) node the SQL serializer renders.
///
/// A path into a document-stored embed (`preferences().theme()`) is a plain
/// positional `ExprProject` everywhere in the engine — byte-for-byte the same
/// shape a column-expanded embed uses — so the in-memory interpreter can
/// evaluate it against the loaded `Value::Record` with no JSON involved.
/// Turning it into a JSON function is purely a SQL-driver concern, so it
/// happens here, the last engine-side step before a driver serializes the
/// statement, rather than in the shared simplifier (which runs for every
/// backend) or in the serializer (which is the driver's job). The driver
/// receives only the `FuncJsonExtract` and renders it per dialect.
fn lower_document_paths(stmt: &mut stmt::Statement, schema: &Schema) {
    LowerDocumentPaths {
        cx: stmt::ExprContext::new(schema),
    }
    .visit_mut(stmt);
}

/// Scoped traversal backing [`lower_document_paths`]. Mirrors the simplifier's
/// scope handling — holding a query's source in scope while mutating its
/// sibling clauses — so a document column reference inside a filter resolves to
/// its `Type::Document`.
struct LowerDocumentPaths<'a> {
    cx: stmt::ExprContext<'a>,
}

impl LowerDocumentPaths<'_> {
    fn scope<'s>(&'s self, target: impl IntoExprTarget<'s>) -> LowerDocumentPaths<'s> {
        LowerDocumentPaths {
            cx: self.cx.scope(target),
        }
    }
}

impl VisitMut for LowerDocumentPaths<'_> {
    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        stmt::visit_mut::visit_expr_mut(self, expr);

        let stmt::Expr::Project(project) = expr else {
            return;
        };
        // Only a projection rooted at a column reference can be a document
        // path; anything else is left for later phases.
        if !matches!(project.base.as_ref(), stmt::Expr::Reference(_)) {
            return;
        }
        let stmt::Type::Document(doc) = self.cx.infer_expr_ty(project.base.as_ref(), &[]) else {
            return;
        };
        let Some((path, ty)) = build_json_path(&doc, project.projection.as_slice()) else {
            return;
        };
        let base = Box::new(project.base.take());
        *expr = stmt::Expr::from(stmt::FuncJsonExtract { base, path, ty });
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        self.visit_source_mut(&mut stmt.source);
        let mut s = self.scope(&stmt.source);
        s.visit_filter_mut(&mut stmt.filter);
        s.visit_returning_mut(&mut stmt.returning);
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        self.visit_source_mut(&mut stmt.from);
        let mut s = self.scope(&stmt.from);
        s.visit_filter_mut(&mut stmt.filter);
        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        self.visit_update_target_mut(&mut stmt.target);
        let mut s = self.scope(&stmt.target);
        s.visit_assignments_mut(&mut stmt.assignments);
        s.visit_filter_mut(&mut stmt.filter);
        if let Some(expr) = &mut stmt.condition.expr {
            s.visit_expr_mut(expr);
        }
        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        self.visit_insert_target_mut(&mut stmt.target);
        let mut s = self.scope(&stmt.target);
        s.visit_stmt_query_mut(&mut stmt.source);
        if let Some(returning) = &mut stmt.returning {
            s.visit_returning_mut(returning);
        }
    }
}

/// Walks a projection through (possibly nested) document types, collecting the
/// key path and the leaf field's type. Returns `None` if any step is out of
/// range.
fn build_json_path(
    doc: &stmt::TypeDocument,
    projection: &[usize],
) -> Option<(Vec<String>, stmt::Type)> {
    let mut current = doc;
    let mut path = Vec::with_capacity(projection.len());
    let mut leaf_ty = None;

    for &index in projection {
        let field = current.fields.get(index)?;
        path.push(field.name.clone());
        if let stmt::Type::Document(nested) = &field.ty {
            current = nested;
        }
        leaf_ty = Some(field.ty.clone());
    }

    Some((path, leaf_ty?))
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
        glob_starts_with: bool,
        binary_like_starts_with: bool,
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
                // `IN (...)` always renders as N separate placeholders on
                // backends without `predicate_match_any` (the ones that
                // could rewrite it to ANY have already done so during
                // lowering). Force per-element expansion of the rhs list,
                // regardless of `bind_list_param`, so `id IN (1, 2, 3)`
                // doesn't degrade to `id IN ?` after we enable `bind_list_param`
                // for backends that bind `Vec<scalar>` columns as one param.
                stmt::Expr::InList(e) => {
                    self.visit_expr_mut(&mut e.expr);
                    if let stmt::Expr::Value(stmt::Value::List(_)) = e.list.as_ref() {
                        let stmt::Expr::Value(stmt::Value::List(items)) =
                            std::mem::replace(e.list.as_mut(), stmt::Expr::null())
                        else {
                            unreachable!()
                        };
                        let items = items
                            .into_iter()
                            .map(|v| value_to_extracted_expr(v, self.params, false))
                            .collect();
                        *e.list = stmt::Expr::List(stmt::ExprList { items });
                    } else {
                        self.visit_expr_mut(&mut e.list);
                    }
                    return;
                }
                // For SQLite/MySQL, transform the prefix into the final search
                // pattern before binding it: GLOB needs `*`/`?`/`[` escaped and
                // a `*` appended; BINARY LIKE needs `%`/`_`/`!` escaped and a
                // `%` appended.  The column expression is visited normally.
                stmt::Expr::StartsWith(e)
                    if self.glob_starts_with || self.binary_like_starts_with =>
                {
                    self.visit_expr_mut(&mut e.expr);
                    let stmt::Expr::Value(stmt::Value::String(prefix)) = e.prefix.as_ref() else {
                        panic!("starts_with prefix must be a string literal");
                    };
                    let pattern = if self.glob_starts_with {
                        glob_prefix_pattern(prefix)
                    } else {
                        binary_like_prefix_pattern(prefix)
                    };
                    let position = self.params.len();
                    self.params.push(Param {
                        value: stmt::Value::String(pattern),
                        ty: Ty::Inferred(db::Type::Text),
                    });
                    *e.prefix = stmt::Expr::arg(position);
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
                // A bare `#[document]` embed value (already rewritten to a
                // named object by `mark_document_values`) binds as one param;
                // `refine` pins its type to the document column.
                stmt::Expr::Value(value @ stmt::Value::Object(_)) => {
                    let owned = std::mem::replace(value, stmt::Value::Null);
                    let position = self.params.len();
                    self.params.push(Param {
                        value: owned,
                        ty: Ty::Unknown,
                    });
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
        glob_starts_with: capability.glob_starts_with,
        binary_like_starts_with: capability.binary_like_starts_with,
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
            if bind_list_param
                && values.iter().all(|v| {
                    // A `Vec<scalar>` collection, or a `#[document]`
                    // collection of embedded structs (`Value::Record`
                    // elements). Either way the whole list binds as one
                    // parameter; `refine` resolves which storage applies and
                    // — for documents — rewrites the records into objects.
                    is_extractable_scalar(v) || matches!(v, stmt::Value::Record(_))
                }) =>
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
    let stmt::InsertTarget::Table(table) = &insert.target else {
        return;
    };
    let db_table = &db_schema.tables[table.table.0];

    // Build expected type from column list (authoritative). Borrow the
    // per-column types back out so the per-row loop below can reuse them
    // instead of re-deriving each column's `Ty` once per VALUES row.
    let expected = Ty::Record(
        table
            .columns
            .iter()
            .map(|col_id| ty_from_column(db_table.columns[col_id.index].storage_ty.clone()))
            .collect(),
    );
    let Ty::Record(field_types) = &expected else {
        unreachable!()
    };

    // Push column types down into each VALUES row
    if let stmt::ExprSet::Values(values) = &insert.source.body {
        for row in &values.rows {
            // A row of `#[document]` columns can't go through the generic
            // numeric/list `check` — its `Value::List(Record)` param has no
            // shape the column type merges with. Handle those columns
            // explicitly, field by field, and let `check` cover the rest.
            if let stmt::Expr::Record(record) = row {
                for ((col_id, field_expr), field_ty) in
                    table.columns.iter().zip(&record.fields).zip(field_types)
                {
                    let col = &db_table.columns[col_id.index];
                    if let Some(doc) = document_elem_ty(&col.ty) {
                        refine_document_param(field_expr, doc, col.storage_ty.clone(), params);
                    } else {
                        check(field_expr, field_ty, params);
                    }
                }
            } else {
                check(row, &expected, params);
            }
        }
    }
}

/// If `ty` is the engine type of a `#[document]` collection column —
/// `List(Document(..))` — return the inner [`stmt::TypeDocument`].
fn document_elem_ty(ty: &stmt::Type) -> Option<&stmt::TypeDocument> {
    match ty {
        // A bare `#[document]` embed column.
        stmt::Type::Document(doc) => Some(doc),
        // A `#[document]` collection column — `List(Document(..))`.
        stmt::Type::List(elem) => match &**elem {
            stmt::Type::Document(doc) => Some(doc),
            _ => None,
        },
        _ => None,
    }
}

/// Refine a `#[document]` collection param: pin its `db::Type` to the document
/// storage type and rewrite the value from the positional `Value::List(Record)`
/// shape into the named `Value::List(Object)` shape the driver serializes.
/// `doc` supplies the field names.
fn refine_document_param(
    expr: &stmt::Expr,
    doc: &stmt::TypeDocument,
    storage_ty: db::Type,
    params: &mut [Param],
) {
    // `None` for an `Option<Vec<..>>` field stays an `Expr::Value(Null)` —
    // it is never extracted as a param, so there's nothing to refine.
    let stmt::Expr::Arg(arg) = expr else {
        return;
    };
    let param = &mut params[arg.position];
    param.ty = Ty::Column(storage_ty);
    let value = std::mem::replace(&mut param.value, stmt::Value::Null);
    // A collection column holds a `Value::List` of records; a bare embed
    // column holds a single `Value::Record`. Convert whichever shape arrives
    // into the named `Value::Object` form the driver serializes.
    param.value = match value {
        stmt::Value::List(_) => document_list_value(value, doc),
        _ => document_record_value(value, doc),
    };
}

/// Convert a `Value::List` of positional `Value::Record`s into a `Value::List`
/// of named `Value::Object`s using the document's field names. A non-list
/// value (`Null`) passes through unchanged.
fn document_list_value(value: stmt::Value, doc: &stmt::TypeDocument) -> stmt::Value {
    match value {
        stmt::Value::List(items) => stmt::Value::List(
            items
                .into_iter()
                .map(|item| document_record_value(item, doc))
                .collect(),
        ),
        other => other,
    }
}

/// Convert one positional `Value::Record` into a named `Value::Object`,
/// recursing through nested document fields.
fn document_record_value(value: stmt::Value, doc: &stmt::TypeDocument) -> stmt::Value {
    let stmt::Value::Record(record) = value else {
        return value;
    };
    stmt::Value::Object(stmt::ValueObject::from_vec(
        doc.fields
            .iter()
            .zip(record)
            .map(|(field, v)| {
                let v = match &field.ty {
                    stmt::Type::Document(nested) => document_record_value(v, nested),
                    stmt::Type::List(elem) => match &**elem {
                        stmt::Type::Document(nested) => document_list_value(v, nested),
                        _ => v,
                    },
                    _ => v,
                };
                (field.name.clone(), v)
            })
            .collect(),
    ))
}

/// Fold a constant value expression into a [`stmt::Value`]. Returns `None` if
/// any sub-expression is non-constant (e.g. a column reference or `Default`),
/// in which case the caller leaves the expression on the generic path.
fn const_value_of(expr: &stmt::Expr) -> Option<stmt::Value> {
    match expr {
        stmt::Expr::Value(value) => Some(value.clone()),
        stmt::Expr::Record(record) => Some(stmt::Value::Record(stmt::ValueRecord::from_vec(
            record
                .fields
                .iter()
                .map(const_value_of)
                .collect::<Option<Vec<_>>>()?,
        ))),
        stmt::Expr::List(list) => Some(stmt::Value::List(
            list.items
                .iter()
                .map(const_value_of)
                .collect::<Option<Vec<_>>>()?,
        )),
        _ => None,
    }
}

/// Rewrite bare `#[document]` embed column values into the named
/// `Value::Object` form, in place, before generic param extraction. A
/// collection column (`List(Document)`) is left for the existing list path; a
/// non-constant document value is left untouched.
fn mark_document_values(stmt: &mut stmt::Statement, db_schema: &db::Schema) {
    // Convert a constant document value *expression* into the named object form.
    fn mark_expr(field_expr: &mut stmt::Expr, doc: &stmt::TypeDocument) {
        if let Some(value) = const_value_of(field_expr) {
            *field_expr = stmt::Expr::Value(document_record_value(value, doc));
        }
    }

    match stmt {
        stmt::Statement::Insert(insert) => {
            let stmt::InsertTarget::Table(table) = &insert.target else {
                return;
            };
            let db_table = &db_schema.tables[table.table.0];
            // Per-column document type, `None` for non-document columns.
            let docs: Vec<Option<&stmt::TypeDocument>> = table
                .columns
                .iter()
                .map(|c| match &db_table.columns[c.index].ty {
                    stmt::Type::Document(doc) => Some(doc),
                    _ => None,
                })
                .collect();
            let stmt::ExprSet::Values(values) = &mut insert.source.body else {
                return;
            };
            for row in &mut values.rows {
                match row {
                    // A row of per-column value expressions.
                    stmt::Expr::Record(record) => {
                        for (doc, field) in docs.iter().zip(&mut record.fields) {
                            if let Some(doc) = doc {
                                mark_expr(field, doc);
                            }
                        }
                    }
                    // A fully-constant row folded to a single record value.
                    stmt::Expr::Value(stmt::Value::Record(record)) => {
                        for (doc, field) in docs.iter().zip(&mut record.fields) {
                            if let Some(doc) = doc {
                                let v = std::mem::replace(field, stmt::Value::Null);
                                *field = document_record_value(v, doc);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        stmt::Statement::Update(update) => {
            let stmt::UpdateTarget::Table(table_id) = &update.target else {
                return;
            };
            let db_table = &db_schema.tables[table_id.0];
            for (projection, assignment) in update.assignments.iter_mut() {
                let steps = projection.as_slice();
                if steps.len() != 1 {
                    continue;
                }
                let Some(col) = db_table.columns.get(steps[0]) else {
                    continue;
                };
                let stmt::Type::Document(doc) = &col.ty else {
                    continue;
                };
                if let stmt::Assignment::Set(expr) = assignment {
                    mark_expr(expr, doc);
                }
            }
        }
        _ => {}
    }
}

fn refine_update(update: &stmt::Update, cx: &Cx<'_>, db_schema: &db::Schema, params: &mut [Param]) {
    // Refine assignment types from target columns
    if let stmt::UpdateTarget::Table(table_id) = &update.target {
        let db_table = &db_schema.tables[table_id.0];

        for (projection, assignment) in update.assignments.iter() {
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
                // The expression takes the column's full type (column for
                // `Set`, list-shaped for `Append`).
                stmt::Assignment::Set(expr) | stmt::Assignment::Append(expr) => {
                    if let Some(doc) = document_elem_ty(&col.ty) {
                        // A whole-value write to a `#[document]` collection
                        // column: bypass the generic numeric/list inference
                        // and rewrite the param into the document shape.
                        refine_document_param(expr, doc, col.storage_ty.clone(), params);
                    } else {
                        let expected = ty_from_column(col.storage_ty.clone());
                        check(expr, &expected, params);
                    }
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

    // Refine filter types
    refine_filter(&update.filter, cx, params);
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

/// Build a SQLite GLOB pattern for a `starts_with(prefix)` expression.
///
/// GLOB metacharacters (`*`, `?`, `[`) are escaped by wrapping them in a
/// bracket class: `*` → `[*]`, `?` → `[?]`, `[` → `[[]`. A trailing `*`
/// wildcard is appended so the pattern matches any string starting with
/// `prefix`. GLOB has no ESCAPE clause, so bracket-class escaping is the only
/// available mechanism.
fn glob_prefix_pattern(prefix: &str) -> String {
    // Each metachar expands to 3 chars; over-allocate slightly rather than
    // under-allocate and trigger a realloc on prefixes with wildcards.
    let mut pattern = String::with_capacity(prefix.len() * 3 + 1);
    for c in prefix.chars() {
        match c {
            '*' => pattern.push_str("[*]"),
            '?' => pattern.push_str("[?]"),
            '[' => pattern.push_str("[[]"),
            c => pattern.push(c),
        }
    }
    pattern.push('*');
    pattern
}

/// Build a MySQL `BINARY col LIKE ? ESCAPE '!'` pattern for `starts_with(prefix)`.
///
/// `!` is the hardcoded escape character. In a single pass, `!`, `%`, and `_`
/// are all prefixed with `!` (so `!` → `!!`, `%` → `!%`, `_` → `!_`). A
/// trailing `%` wildcard is appended.
fn binary_like_prefix_pattern(prefix: &str) -> String {
    let mut pattern = String::with_capacity(prefix.len() + 1);
    for c in prefix.chars() {
        match c {
            '!' | '%' | '_' => {
                pattern.push('!');
                pattern.push(c);
            }
            c => pattern.push(c),
        }
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests;
