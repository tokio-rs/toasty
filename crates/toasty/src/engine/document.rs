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
    schema::{Schema, app},
    stmt::{self, IntoExprTarget, VisitMut},
};

/// Lower every `#[document]` column in `stmt` into its driver-serializable
/// shape: path reads become `FuncJsonExtract`, write values become
/// `Value::Object`.
pub(crate) fn lower(schema: &Schema, stmt: &mut stmt::Statement) {
    lower_paths(schema, stmt);
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
fn lower_paths(schema: &Schema, stmt: &mut stmt::Statement) {
    LowerDocumentPaths {
        cx: stmt::ExprContext::new(schema),
    }
    .visit_mut(stmt);
}

/// Scoped traversal backing [`lower_paths`]. Mirrors the simplifier's scope
/// handling — holding a query's source in scope while mutating its sibling
/// clauses — so a document column reference inside a filter resolves to its
/// embedded-model type (`Type::Model`).
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
    schema: &Schema,
    embed_id: app::ModelId,
    projection: &[usize],
) -> Option<(Vec<String>, stmt::Type)> {
    let mut current = embed_id;
    let mut path = Vec::with_capacity(projection.len());
    let mut leaf_ty = None;

    for &index in projection {
        let (name, ty) = schema.app.document_fields(current).nth(index)?;
        path.push(name.to_owned());
        if let stmt::Type::Model(nested) = ty {
            current = *nested;
        }
        leaf_ty = Some(ty.clone());
    }

    Some((path, leaf_ty?))
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
