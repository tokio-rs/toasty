//! Lower `#[document]` path reads into the shape drivers consume, at the
//! engine/driver boundary.
//!
//! The engine views a document column at the app level: the mapping's raising
//! cast (`Cast(column, Type::Model)`) turns the driver's named `Value::Object`
//! into the embed's positional `Value::Record`, and a path into the document
//! is a positional projection through that cast —
//! `Project(Cast(column, Model), steps)` — which the in-memory interpreter
//! evaluates directly. Drivers and the database schema see only the
//! structural view: the column is typed `Type::Object`, its value is a named
//! `Value::Object`, and a path is a resolved [`stmt::FuncJsonExtract`] name
//! path. The model identity never crosses the driver boundary.
//!
//! Value conversion between the two views is the mapping's job: the schema
//! builder plants a lowering cast (`Model` → `Object`, carrying the source
//! type — see `stmt::ExprCast::from`) in `model_to_table` and the raising
//! cast in `table_to_model`, and the cast machinery (fold/simplify constant
//! folding, `mir::Project` evaluation) converts values wherever the casts
//! land. What cannot be expressed as a cast is the *path read*: turning a
//! positional projection into a JSON key path is a per-backend expression
//! rewrite (the leaf's comparison form depends on driver capabilities), so it
//! runs here — the last engine-side step before a driver serializes the
//! statement, rather than in the shared simplifier (which runs for every
//! backend) or in the serializer (which is the driver's job).

use toasty_core::{
    driver::Capability,
    schema::{Schema, app, db},
    stmt::{self, IntoExprTarget, VisitMut},
};

/// Rewrite every projection into a `#[document]` column into the
/// [`FuncJsonExtract`](stmt::FuncJsonExtract) node the SQL serializer
/// renders, and strip any residual raising cast (meaningless driver-side).
pub(crate) fn lower_paths(schema: &Schema, capability: &Capability, stmt: &mut stmt::Statement) {
    LowerDocumentPaths {
        cx: stmt::ExprContext::new(schema),
        capability,
    }
    .visit_mut(stmt);
}

/// Lower a driver-bound expression that references `table`'s columns — a
/// key-value operation's filter or condition — into its driver-consumable
/// shape: document paths become [`stmt::FuncJsonExtract`] name paths, and
/// text-compared document leaves get text operands, exactly as
/// [`lower_paths`] does for full statements.
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

/// Scoped traversal backing [`lower_paths`]. Mirrors the simplifier's scope
/// handling — holding a query's source in scope while mutating its sibling
/// clauses — so column references resolve against the right table.
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
    ///
    /// The lowered form of a document path is a positional projection through
    /// the column's raising cast — `Project(Cast(column, Model), steps)`.
    /// Driver-side there is no positional form: the projection becomes a JSON
    /// key path rooted at the bare column reference.
    fn lower_project(&self, expr: &mut stmt::Expr) {
        let stmt::Expr::Project(project) = expr else {
            return;
        };
        // Only a projection through a document raising cast rooted at a
        // column reference is a document path; anything else is left for
        // later phases.
        let stmt::Expr::Cast(cast) = &mut *project.base else {
            return;
        };
        let stmt::Type::Model(embed_id) = cast.ty else {
            return;
        };
        if !matches!(&*cast.expr, stmt::Expr::Reference(_)) {
            return;
        }
        let Some((path, ty)) =
            build_json_path(self.cx.schema(), embed_id, project.projection.as_slice())
        else {
            return;
        };
        let base = Box::new(cast.expr.take());
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
        // A document path must be lowered before descending into its base:
        // the base is the raising cast that names the embed, which the
        // residual cast strip below would otherwise remove first (children
        // are visited before their parent).
        if let stmt::Expr::Project(_) = expr {
            self.lower_project(expr);
        }

        stmt::visit_mut::visit_expr_mut(self, expr);

        // Children are visited first, so by the time a comparison node is
        // reached its document-path side is already a `FuncJsonExtract`.
        match expr {
            // A raising cast left outside a document path (a whole-document
            // reference) is meaningless driver-side: the column already
            // carries the named object. Strip it to the bare reference.
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.contains_model() => {
                *expr = expr_cast.expr.take();
            }
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
