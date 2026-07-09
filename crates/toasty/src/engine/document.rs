//! Convert `#[document]` values and expressions between the engine's
//! model-level form and the structural form drivers consume, at the
//! engine/driver boundary.
//!
//! The engine views a document column at the app level: its type is
//! `Type::Model`, its value a positional `Value::Record`, and a path into it
//! a plain positional `ExprProject`. Drivers and the database schema see only
//! the structural view: the column is typed `Type::Object`, the value is a
//! named `Value::Object`, and a path is a resolved [`stmt::FuncJsonExtract`]
//! name path. The model identity never crosses the driver boundary; this
//! module is where the two views convert:
//!
//! - **lowering** (engine → driver, [`lower`]): projections into a document
//!   column become `FuncJsonExtract` nodes, and write values become named
//!   `Value::Object`s. Runs last — after planning, just before the driver —
//!   because the in-memory interpreter wants the positional form.
//! - **raising** (driver → engine, [`raise`]): a document value decoded
//!   shape-directed by a driver (a named object with wire-shaped leaves)
//!   becomes the typed positional `Value::Record` the engine consumes,
//!   resolved against the embedded model's schema. Runs first — on rows as
//!   they come back from the driver, before any in-memory evaluation.

use toasty_core::{
    driver::Capability,
    schema::{Schema, app, db},
    stmt::{self, IntoExprTarget, VisitMut},
};

/// Lower every `#[document]` column in `stmt` into its driver-serializable
/// shape: path reads become `FuncJsonExtract`, write values become
/// `Value::Object`.
pub(crate) fn lower(schema: &Schema, capability: &Capability, stmt: &mut stmt::Statement) {
    lower_paths(schema, capability, stmt);
    name_values(schema, stmt);
}

// ============================================================================
// Path reads → JSON extraction
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
fn lower_paths(schema: &Schema, capability: &Capability, stmt: &mut stmt::Statement) {
    LowerDocumentPaths {
        cx: stmt::ExprContext::new(schema),
        capability,
    }
    .visit_mut(stmt);
}

/// Scoped traversal backing [`lower_paths`]. Mirrors the simplifier's scope
/// handling — holding a query's source in scope while mutating its sibling
/// clauses — so a document column reference inside a filter resolves to its
/// embedded-model type (`Type::Model`).
struct LowerDocumentPaths<'a> {
    cx: stmt::ExprContext<'a>,
    capability: &'a Capability,
}

impl LowerDocumentPaths<'_> {
    fn scope<'s>(&'s self, target: impl IntoExprTarget<'s>) -> LowerDocumentPaths<'s> {
        LowerDocumentPaths {
            cx: self.cx.scope(target),
            capability: self.capability,
        }
    }

    /// Rewrite a projection rooted at a `#[document]` column into the
    /// [`FuncJsonExtract`](stmt::FuncJsonExtract) the SQL serializer renders.
    fn lower_project(&self, expr: &mut stmt::Expr) {
        let stmt::Expr::Project(project) = expr else {
            return;
        };
        // Only a projection rooted at a column reference can be a document
        // path; anything else is left for later phases.
        if !matches!(project.base.as_ref(), stmt::Expr::Reference(_)) {
            return;
        }
        let stmt::Type::Model(embed_id) = self.cx.infer_expr_ty(project.base.as_ref(), &[]) else {
            return;
        };
        let Some((path, ty)) =
            build_json_path(self.cx.schema(), embed_id, project.projection.as_slice())
        else {
            return;
        };
        let base = Box::new(project.base.take());
        *expr = stmt::Expr::from(stmt::FuncJsonExtract { base, path, ty });
    }

    /// Whether a document leaf of type `ty` compares as plain text on this
    /// backend. PostgreSQL and MySQL cast the JSON extraction back to the
    /// leaf's native SQL type, but a backend without that native type (SQLite
    /// has no temporal or decimal column types) compares the extracted text
    /// directly — so the bound operand must be the exact text the JSON codec
    /// stores.
    fn leaf_compares_as_text(&self, ty: &stmt::Type) -> bool {
        match ty {
            #[cfg(feature = "jiff")]
            stmt::Type::Timestamp => !self.capability.native_timestamp,
            #[cfg(feature = "jiff")]
            stmt::Type::Date => !self.capability.native_date,
            #[cfg(feature = "jiff")]
            stmt::Type::Time => !self.capability.native_time,
            #[cfg(feature = "jiff")]
            stmt::Type::DateTime => !self.capability.native_datetime,
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => !self.capability.native_decimal,
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => !self.capability.native_decimal,
            _ => false,
        }
    }

    /// If `extract_side` is a JSON extraction whose leaf compares as text on
    /// this backend, rewrite the constant `operand` to the leaf's document
    /// text form (the exact text the JSON codec stores — see
    /// [`document_text`]) and retype the extraction as a text read.
    fn textify_comparison(&self, extract_side: &mut stmt::Expr, operand: &mut stmt::Expr) {
        let stmt::Expr::Func(stmt::ExprFunc::JsonExtract(func)) = extract_side else {
            return;
        };
        if !self.leaf_compares_as_text(&func.ty) {
            return;
        }
        let stmt::Expr::Value(value) = operand else {
            return;
        };
        let Some(text) = document_text(value) else {
            return;
        };
        *value = stmt::Value::String(text);
        func.ty = stmt::Type::String;
    }
}

impl VisitMut for LowerDocumentPaths<'_> {
    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        stmt::visit_mut::visit_expr_mut(self, expr);

        // Children are visited first, so by the time a comparison node is
        // reached its document-path side is already a `FuncJsonExtract`.
        match expr {
            stmt::Expr::Project(_) => self.lower_project(expr),
            stmt::Expr::BinaryOp(binary) => {
                self.textify_comparison(&mut binary.lhs, &mut binary.rhs);
                self.textify_comparison(&mut binary.rhs, &mut binary.lhs);
            }
            stmt::Expr::InList(in_list) => {
                let stmt::Expr::Func(stmt::ExprFunc::JsonExtract(func)) = &mut *in_list.expr else {
                    return;
                };
                if !self.leaf_compares_as_text(&func.ty) {
                    return;
                }
                match &mut *in_list.list {
                    stmt::Expr::List(list) => {
                        for item in &mut list.items {
                            if let stmt::Expr::Value(value) = item
                                && let Some(text) = document_text(value)
                            {
                                *value = stmt::Value::String(text);
                            }
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(items)) => {
                        for value in items.iter_mut() {
                            if let Some(text) = document_text(value) {
                                *value = stmt::Value::String(text);
                            }
                        }
                    }
                    _ => return,
                }
                func.ty = stmt::Type::String;
            }
            _ => {}
        }
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

/// Resolves a projection through (possibly nested) document types via the
/// schema's shared [`project_fields`](app::Schema::project_fields) walk,
/// collecting the JSON key path and the leaf field's type. Returns `None` if
/// the projection does not resolve to a document path.
fn build_json_path(
    schema: &Schema,
    embed_id: app::ModelId,
    projection: &[usize],
) -> Option<(Vec<String>, stmt::Type)> {
    let mut path = Vec::with_capacity(projection.len());
    let mut leaf_ty = None;

    for field in schema.app.project_fields(embed_id, projection) {
        path.push(field.name.app.as_deref()?.to_owned());
        leaf_ty = Some(field.expr_ty().clone());
    }

    // `project_fields` yields fewer fields than asked when the projection does
    // not resolve to a document path (an out-of-range step, or a descent past
    // a non-document leaf).
    (!path.is_empty() && path.len() == projection.len())
        .then(|| (path, leaf_ty.expect("a non-empty path has a leaf type")))
}

// ============================================================================
// Write values → named objects
// ============================================================================

/// Rewrite every `#[document]` column value — a bare embed (`Type::Model`) or a
/// collection (`List(Model)`) — into the named `Value::Object` form, in place.
/// This is the *single* place document values are named; later phases only type
/// the param, never reshape the value. A non-constant document value is left
/// untouched (document writes are constant literals via `create!` / `IntoExpr`,
/// so this only matters defensively).
fn name_values(schema: &Schema, stmt: &mut stmt::Statement) {
    // Convert a constant document value *expression* into the named object
    // form, directed by the column type (`Type::Model` or `List(Model)`).
    fn mark_expr(app: &app::Schema, field_expr: &mut stmt::Expr, ty: &stmt::Type) {
        if let Some(value) = const_value_of(field_expr) {
            *field_expr = stmt::Expr::Value(to_named(app, value, ty));
        }
    }

    match stmt {
        stmt::Statement::Insert(insert) => {
            let stmt::InsertTarget::Table(table) = &insert.target else {
                return;
            };
            let db_table = &schema.db.tables[table.table.0];
            // Per-column document type (`Type::Model` or `List(Model)`), `None`
            // for non-document columns. The column itself is typed by the
            // structural `Type::Object`; its model identity lives in the
            // mapping.
            let docs: Vec<Option<&stmt::Type>> = table
                .columns
                .iter()
                .map(|c| {
                    schema
                        .mapping
                        .document_column_ty(db_table.columns[c.index].id)
                })
                .collect();
            let stmt::ExprSet::Values(values) = &mut insert.source.body else {
                return;
            };
            for row in &mut values.rows {
                match row {
                    // A row of per-column value expressions.
                    stmt::Expr::Record(record) => {
                        for (ty, field) in docs.iter().zip(&mut record.fields) {
                            if let Some(ty) = ty {
                                mark_expr(&schema.app, field, ty);
                            }
                        }
                    }
                    // A fully-constant row folded to a single record value.
                    stmt::Expr::Value(stmt::Value::Record(record)) => {
                        for (ty, field) in docs.iter().zip(&mut record.fields) {
                            if let Some(ty) = ty {
                                let v = std::mem::replace(field, stmt::Value::Null);
                                *field = to_named(&schema.app, v, ty);
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
            let db_table = &schema.db.tables[table_id.0];
            name_assignment_values(schema, db_table, &mut update.assignments);
        }
        _ => {}
    }
}

/// Rewrite a positional document value into the named `Value::Object` form the
/// driver serializes, directed by the column's document `ty`. The value's shape
/// always matches `ty`: a `Type::Model` embed carries a `Value::Record`, a
/// `List(Model)` collection carries a `Value::List` (a whole-collection `Set`,
/// or an `Append`, which `push`/`extend` always wrap in a list).
///
/// - a `Type::Model` embed turns the `Value::Record` into a `Value::Object`,
///   resolving the embed's field names from the schema and recursing.
/// - `List(Model)` maps each element through the element type.
/// - anything else — a non-document field, or an already-`Object`/`Null` value —
///   passes through unchanged, so the conversion is idempotent.
fn to_named(app: &app::Schema, value: stmt::Value, ty: &stmt::Type) -> stmt::Value {
    match (ty, value) {
        (stmt::Type::Model(embed_id), stmt::Value::Record(record)) => {
            stmt::Value::Object(stmt::ValueObject::from_vec(
                app.fields(*embed_id)
                    .iter()
                    .zip(record)
                    .map(|(field, v)| {
                        (
                            field.name().app_unwrap().to_owned(),
                            to_named(app, v, field.expr_ty()),
                        )
                    })
                    .collect(),
            ))
        }
        (stmt::Type::List(elem), stmt::Value::List(items)) => stmt::Value::List(
            items
                .into_iter()
                .map(|item| to_named(app, item, elem))
                .collect(),
        ),
        (_, value) => value,
    }
}

// ============================================================================
// Key-value operations: driver-bound expressions and assignments
// ============================================================================

/// Lower a driver-bound expression that references `table`'s columns — a
/// key-value operation's filter or condition — into its driver-consumable
/// shape: document paths become [`stmt::FuncJsonExtract`] name paths, and
/// text-compared document leaves get text operands, exactly as [`lower`] does
/// for full statements.
pub(crate) fn lower_table_expr(
    schema: &Schema,
    capability: &Capability,
    table: &db::Table,
    expr: &mut stmt::Expr,
) {
    LowerDocumentPaths {
        cx: stmt::ExprContext::new_with_target(schema, table),
        capability,
    }
    .visit_expr_mut(expr);
}

/// Name the document values in a key-value update's assignments — the same
/// conversion [`name_values`] applies to a table `UPDATE` statement's
/// assignments.
pub(crate) fn name_assignment_values(
    schema: &Schema,
    table: &db::Table,
    assignments: &mut stmt::Assignments,
) {
    for (projection, assignment) in assignments.iter_mut() {
        let steps = projection.as_slice();
        if steps.len() != 1 {
            continue;
        }
        let Some(col) = table.columns.get(steps[0]) else {
            continue;
        };
        let Some(doc_ty) = schema.mapping.document_column_ty(col.id) else {
            continue;
        };
        // `Set` carries the whole column value; `Append` carries the elements
        // to add, which `push`/`extend` always wrap in a list. Either way the
        // value matches the column's document type.
        if let stmt::Assignment::Set(expr) | stmt::Assignment::Append(expr) = assignment
            && let Some(value) = const_value_of(expr)
        {
            *expr = stmt::Expr::Value(to_named(&schema.app, value, doc_ty));
        }
    }
}

// ============================================================================
// Driver-facing result types
// ============================================================================

/// The driver-facing form of an engine-level result type: every document
/// position (`Type::Model`) becomes the structural `Type::Object` a driver
/// decodes shape-directed. This is the type-level counterpart of
/// [`to_named`]; the model identity stays engine-side.
pub(crate) fn lower_ty(ty: stmt::Type) -> stmt::Type {
    match ty {
        stmt::Type::Model(_) => stmt::Type::Object,
        stmt::Type::List(elem) => stmt::Type::List(Box::new(lower_ty(*elem))),
        stmt::Type::Record(fields) => {
            stmt::Type::Record(fields.into_iter().map(lower_ty).collect())
        }
        ty => ty,
    }
}

// ============================================================================
// Read values: wire objects → typed positional records
// ============================================================================

/// Whether `ty` has a document position (`Type::Model`) that a driver row
/// could carry in wire form. Callers use this to skip the [`raise`] walk
/// entirely for result shapes that cannot contain documents.
pub(crate) fn ty_contains_document(ty: &stmt::Type) -> bool {
    match ty {
        stmt::Type::Model(_) => true,
        stmt::Type::List(elem) => ty_contains_document(elem),
        stmt::Type::Record(fields) => fields.iter().any(ty_contains_document),
        stmt::Type::Union(union) => union.iter().any(ty_contains_document),
        _ => false,
    }
}

/// Raise a driver-returned value into the engine's typed, positional form,
/// directed by the engine-level `ty` — the inverse of [`to_named`].
///
/// Drivers decode a `#[document]` column shape-directed: the value arrives as
/// a named `Value::Object` whose leaves have their wire shapes (numbers by
/// integer fit, temporals/decimals/uuids as text). This walk rebuilds the
/// engine form: each `Type::Model` position becomes a positional
/// `Value::Record` in schema field order (an absent key decodes to `Null`),
/// and each document leaf is cast to its field's type.
///
/// Values outside a document position pass through untouched — drivers decode
/// scalar columns type-directed, so only document interiors need conversion.
/// The conversion is idempotent: an already-raised `Value::Record` (an
/// engine-computed row flowing back through a variable) passes through.
pub(crate) fn raise(
    app: &app::Schema,
    ty: &stmt::Type,
    value: stmt::Value,
) -> crate::Result<stmt::Value> {
    if !ty_contains_document(ty) {
        return Ok(value);
    }

    Ok(match (ty, value) {
        (stmt::Type::Model(_), value @ stmt::Value::Object(_)) => raise_document(app, ty, value)?,
        (stmt::Type::List(elem), stmt::Value::List(items)) => stmt::Value::List(
            items
                .into_iter()
                .map(|item| raise(app, elem, item))
                .collect::<crate::Result<_>>()?,
        ),
        (stmt::Type::Record(fields), stmt::Value::Record(record)) => {
            stmt::Value::Record(stmt::ValueRecord::from_vec(
                fields
                    .iter()
                    .zip(record)
                    .map(|(ty, value)| raise(app, ty, value))
                    .collect::<crate::Result<_>>()?,
            ))
        }
        // A union member is picked by shape: raise with the first member the
        // value satisfies (a wire object satisfies its `Type::Model` member
        // via the named field check).
        (stmt::Type::Union(union), value) => match union.iter().find(|ty| value.is_a(app, ty)) {
            Some(ty) => raise(app, ty, value)?,
            None => value,
        },
        (_, value) => value,
    })
}

/// Raise a value at a document position: a named wire object becomes the
/// embed's positional record, descending nested embeds and collections, and
/// casting wire-shaped scalar leaves ([`raise_document_leaf`]).
fn raise_document(
    app: &app::Schema,
    ty: &stmt::Type,
    value: stmt::Value,
) -> crate::Result<stmt::Value> {
    Ok(match (ty, value) {
        (stmt::Type::Model(embed_id), stmt::Value::Object(object)) => {
            let mut entries = object.entries;
            stmt::Value::Record(stmt::ValueRecord::from_vec(
                app.fields(*embed_id)
                    .iter()
                    .map(|field| {
                        let name = field.name().app_unwrap();
                        // Keys the writer omitted (an `Option` leaf holding
                        // `None`) and keys unknown to the schema (written by
                        // an external client, ignored) both fall out here:
                        // the former decode to `Null`, the latter are dropped.
                        match entries.iter().position(|(key, _)| key == name) {
                            Some(index) => raise_document_leaf(
                                app,
                                field.expr_ty(),
                                entries.swap_remove(index).1,
                            ),
                            None => Ok(stmt::Value::Null),
                        }
                    })
                    .collect::<crate::Result<_>>()?,
            ))
        }
        (stmt::Type::List(elem), stmt::Value::List(items)) => stmt::Value::List(
            items
                .into_iter()
                .map(|item| raise_document_leaf(app, elem, item))
                .collect::<crate::Result<_>>()?,
        ),
        // Already in engine form — idempotence for engine-computed values.
        (_, value) => value,
    })
}

/// Raise one document-interior value: descend document structure, pass
/// through leaves already of the field's type, and cast the rest — the wire
/// shapes a shape-directed decode produces (integers by fit, temporals /
/// decimals / uuids as text) back to the field's type, through the same
/// `Type::cast` conversions the encode side uses.
fn raise_document_leaf(
    app: &app::Schema,
    ty: &stmt::Type,
    value: stmt::Value,
) -> crate::Result<stmt::Value> {
    match (ty, value) {
        (stmt::Type::Model(_), value @ stmt::Value::Object(_)) => raise_document(app, ty, value),
        (stmt::Type::List(elem), stmt::Value::List(items)) => Ok(stmt::Value::List(
            items
                .into_iter()
                .map(|item| raise_document_leaf(app, elem, item))
                .collect::<crate::Result<_>>()?,
        )),
        (_, stmt::Value::Null) => Ok(stmt::Value::Null),
        (ty, value) if value.is_a(app, ty) => Ok(value),
        (ty, value) => ty.cast(value),
    }
}

/// The text form `value` takes inside a stored JSON document
/// ([`stmt::Value::document_storage_text`]), for comparison operands bound
/// against a plain-text extraction — exactly what the codec's `collect_str`
/// writes. `None` for values with no document text form (including `Null`,
/// which comparisons reach via `IsNull` instead).
fn document_text(value: &stmt::Value) -> Option<String> {
    #[cfg(any(feature = "jiff", feature = "rust_decimal", feature = "bigdecimal"))]
    {
        value.document_storage_text().map(|text| text.to_string())
    }
    #[cfg(not(any(feature = "jiff", feature = "rust_decimal", feature = "bigdecimal")))]
    {
        let _ = value;
        None
    }
}

/// Fold a constant value expression into a [`stmt::Value`]. Returns `None` if
/// any sub-expression is non-constant (e.g. a column reference or `Default`),
/// in which case the caller leaves the expression untouched.
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
