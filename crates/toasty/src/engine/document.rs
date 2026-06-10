//! Lower a statement's `#[document]` columns into the shape a SQL driver
//! serializes, at the engine/driver boundary.
//!
//! - **reads**: a projection into a document column (`preferences().theme()`)
//!   travels as a plain `ExprProject`; rewrite it to the `FuncJsonExtract` the
//!   serializer renders.
//! - **writes**: a document value is a positional `Value::Record`; rewrite it
//!   to the named `Value::Object` the driver encodes as JSON.
//!
//! Both run last — after planning, just before the driver serializes — because
//! the in-memory interpreter wants the positional/projection form and only the
//! SQL serializer wants the JSON form.

use toasty_core::{
    driver::Capability,
    schema::{Schema, app},
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
/// schema's shared [`document_path_steps`](app::Schema::document_path_steps)
/// walk, collecting the key path and the leaf field's type. Returns `None` if
/// the projection does not resolve to a document path.
fn build_json_path(
    schema: &Schema,
    embed_id: app::ModelId,
    projection: &[usize],
) -> Option<(Vec<String>, stmt::Type)> {
    let steps = schema.app.document_path_steps(embed_id, projection)?;
    let path = steps.iter().map(|(name, _)| (*name).to_owned()).collect();
    let (_, leaf_ty) = steps.last().expect("document path is never empty");
    Some((path, (*leaf_ty).clone()))
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
            // for non-document columns.
            let docs: Vec<Option<&stmt::Type>> = table
                .columns
                .iter()
                .map(|c| {
                    let col = &db_table.columns[c.index];
                    col.is_document().then_some(&col.ty)
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
            for (projection, assignment) in update.assignments.iter_mut() {
                let steps = projection.as_slice();
                if steps.len() != 1 {
                    continue;
                }
                let Some(col) = db_table.columns.get(steps[0]) else {
                    continue;
                };
                if !col.is_document() {
                    continue;
                }
                // `Set` carries the whole column value; `Append` carries the
                // elements to add, which `push`/`extend` always wrap in a list.
                // So for a collection column both are `List`-shaped, and a bare
                // embed only ever takes `Set` — the value always matches
                // `col.ty`, which is what `to_named` is directed by.
                if let stmt::Assignment::Set(expr) | stmt::Assignment::Append(expr) = assignment {
                    mark_expr(&schema.app, expr, &col.ty);
                }
            }
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
                app.document_fields(*embed_id)
                    .zip(record)
                    .map(|((name, field_ty), v)| (name.to_owned(), to_named(app, v, field_ty)))
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

/// The text form `value` takes inside a stored JSON document, for comparison
/// operands bound against a plain-text extraction. Temporal values go through
/// the shared [`stmt::DocumentTemporalText`] form; decimals use their
/// `Display` form — exactly what the codec's `collect_str` writes. `None` for
/// values with no document text form (including `Null`, which comparisons
/// reach via `IsNull` instead).
fn document_text(value: &stmt::Value) -> Option<String> {
    #[cfg(feature = "jiff")]
    if let Some(text) = stmt::DocumentTemporalText::of(value) {
        return Some(text.to_string());
    }
    match value {
        #[cfg(feature = "rust_decimal")]
        stmt::Value::Decimal(v) => Some(v.to_string()),
        #[cfg(feature = "bigdecimal")]
        stmt::Value::BigDecimal(v) => Some(v.to_string()),
        _ => None,
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
