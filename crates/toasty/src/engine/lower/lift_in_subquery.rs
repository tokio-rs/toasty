//! App-level rewrites that lift relation references out of `IN` subqueries
//! into direct foreign-key comparisons.
//!
//! [`LiftInSubquery`] runs as a whole-statement pre-pass before the main
//! lowering walk.  The visitor overrides `visit_expr_mut` to fire two
//! rewrites pre-children:
//!
//! - [`lift_in_subquery`] fires on `Expr::InSubquery` where the LHS is a
//!   relation field reference (`BelongsTo`/`HasOne`/`HasMany`).  For
//!   `BelongsTo` it tries to lift the subquery's filter into FK comparisons
//!   on the parent, falling back to a re-targeted IN subquery on the
//!   foreign-key column.  For `HasOne`/`HasMany` it always rewrites to a
//!   foreign-key IN subquery against the related table.
//!
//! - [`try_lift_relation_path_comparison`] fires on `Expr::BinaryOp` where
//!   one side is `project(ref_self_field(relation_field), [idx, ...])`.
//!   It synthesises a subquery on the target model with the comparison
//!   re-rooted there, then defers to [`lift_in_subquery`].
//!
//! A pre-pass is necessary (rather than folding into
//! `LowerStatement::visit_expr_mut` per #823's pattern) because not every
//! expression that contains an `IN` subquery flows through
//! `LowerStatement::visit_expr_mut`; `ApplyInsertScope::apply_expr` in
//! particular walks insert-scope constraint expressions through its own
//! recursion and would panic on an unlifted relation `IN` subquery.
//!
//! The free functions [`lift_in_subquery`] and
//! [`try_lift_relation_path_comparison`] are exposed on `&ExprContext` so
//! the visitor and the unit tests can both call them without constructing
//! a `LiftInSubquery`.

use toasty_core::{
    schema::app::{BelongsTo, FieldId, FieldTy, ModelId},
    stmt::{self, Expr, ExprContext, IntoExprTarget, ResolvedRef, Visit, VisitMut},
};

/// Pre-lowering pass that lifts relation references out of `IN`-subquery
/// and projection comparisons into direct foreign-key forms.  Runs as a
/// whole-statement visitor before the main lowering walk: code paths that
/// process expressions outside the lowering walk (notably
/// `ApplyInsertScope::apply_expr`) see the already-lifted form.
pub(super) struct LiftInSubquery<'a> {
    cx: ExprContext<'a>,
}

impl<'a> LiftInSubquery<'a> {
    pub(super) fn new(cx: ExprContext<'a>) -> Self {
        Self { cx }
    }

    pub(super) fn rewrite(&mut self, stmt: &mut stmt::Statement) {
        self.visit_mut(stmt);
    }

    fn scope<'scope>(&'scope self, target: impl IntoExprTarget<'scope>) -> LiftInSubquery<'scope> {
        LiftInSubquery {
            cx: self.cx.scope(target),
        }
    }
}

impl VisitMut for LiftInSubquery<'_> {
    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        // Apply lifts pre-children: the rewrites pattern-match on app-
        // level `Reference::Field` to a relation, and the children walk
        // would not introduce relation references that were not there.
        match expr {
            stmt::Expr::InSubquery(e) => {
                if let Some(lifted) = lift_in_subquery(&self.cx, &e.expr, &e.query) {
                    *expr = lifted;
                }
            }
            stmt::Expr::BinaryOp(e) => {
                if let Some(lifted) =
                    try_lift_relation_path_comparison(&self.cx, e.op, &e.lhs, &e.rhs)
                {
                    *expr = lifted;
                } else if let Some(lifted) =
                    try_lift_relation_path_comparison(&self.cx, e.op.commute(), &e.rhs, &e.lhs)
                {
                    *expr = lifted;
                }
            }
            _ => {}
        }

        // Walk children (which may themselves be expressions needing
        // lifts on subtrees).
        stmt::visit_mut::visit_expr_mut(self, expr);
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        self.visit_source_mut(&mut stmt.from);

        let mut s = self.scope(&stmt.from);

        s.visit_filter_mut(&mut stmt.filter);

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

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        self.visit_source_mut(&mut stmt.source);

        let mut s = self.scope(&stmt.source);

        s.visit_filter_mut(&mut stmt.filter);
        s.visit_returning_mut(&mut stmt.returning);
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
}

struct LiftBelongsTo<'a> {
    cx: ExprContext<'a>,
    belongs_to: &'a BelongsTo,
    // TODO: switch to bit field set
    fk_field_matches: Vec<bool>,
    fail: bool,
    operands: Vec<stmt::Expr>,
}

/// Lift `expr IN (subquery)` into a foreign-key-based comparison when
/// `expr` is a relation field reference and `subquery` targets the
/// relation's target model.
///
/// Returns `None` when the LHS is not a relation field reference or when
/// the lift cannot apply.  When the lift succeeds, the returned
/// expression may itself contain unlowered references and the caller is
/// expected to re-visit it through the lowering walk.
pub(super) fn lift_in_subquery(
    cx: &ExprContext,
    expr: &stmt::Expr,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    // The expression is a path expression referencing a relation.
    let field = match expr {
        stmt::Expr::Project(_) => {
            todo!()
        }
        stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Field { .. }) => {
            cx.resolve_expr_reference(expr_reference).as_field_unwrap()
        }
        _ => {
            return None;
        }
    };

    // If the field is not a relation, abort.
    match &field.ty {
        FieldTy::BelongsTo(belongs_to) => lift_belongs_to_in_subquery(cx, belongs_to, query),
        FieldTy::HasOne(has_one) => {
            lift_has_n_in_subquery(has_one.target, has_one.pair(&cx.schema().app), query)
        }
        FieldTy::HasMany(has_many) => {
            lift_has_n_in_subquery(has_many.target, has_many.pair(&cx.schema().app), query)
        }
        _ => None,
    }
}

/// Lift `project(ref_self_field(rel), [idx, ...]) op other` into a
/// foreign-key-based comparison by synthesising an IN subquery on the
/// relation's target model and deferring to [`lift_in_subquery`].
///
/// Returns `None` when `project_side` is not a project through a
/// relation field reference.
pub(super) fn try_lift_relation_path_comparison(
    cx: &ExprContext,
    op: stmt::BinaryOp,
    project_side: &stmt::Expr,
    other_side: &stmt::Expr,
) -> Option<stmt::Expr> {
    let Expr::Project(project_expr) = project_side else {
        return None;
    };
    let Expr::Reference(expr_ref) = &*project_expr.base else {
        return None;
    };
    let ResolvedRef::Field(field) = cx.resolve_expr_reference(expr_ref) else {
        return None;
    };

    let target_model_id = match &field.ty {
        FieldTy::HasOne(rel) => rel.target,
        FieldTy::BelongsTo(rel) => rel.target,
        FieldTy::HasMany(rel) => rel.target,
        _ => return None,
    };

    // Re-root the projection at the target model: the leading index
    // points at the relation field itself, the rest indexes into the
    // related model's fields.
    let (head_idx, tail) = project_expr.projection.as_slice().split_first()?;
    let target_field = Expr::ref_self_field(FieldId {
        model: target_model_id,
        index: *head_idx,
    });
    let target_lhs = if tail.is_empty() {
        target_field
    } else {
        Expr::project(target_field, stmt::Projection::from(tail))
    };

    let subquery = stmt::Query::new_select(
        stmt::Source::from(target_model_id),
        Expr::binary_op(target_lhs, op, other_side.clone()),
    );

    lift_in_subquery(cx, &project_expr.base, &subquery)
}

/// BelongsTo branch: try to lift the subquery's filter into direct FK
/// comparisons.  When the filter references only FK-mapped fields, return
/// the AND of per-FK equalities.  Otherwise, fall back to an IN subquery
/// on the foreign-key column(s) — a tuple-form IN for composite FKs.
///
/// Returns `None` when the subquery does not target the BelongsTo's
/// target model.
fn lift_belongs_to_in_subquery(
    cx: &ExprContext,
    belongs_to: &BelongsTo,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    if belongs_to.target != query.body.as_select_unwrap().source.model_id_unwrap() {
        return None;
    }

    let select = query.body.as_select_unwrap();

    let mut lift = LiftBelongsTo {
        cx: cx.scope(&select.source),
        belongs_to,
        fk_field_matches: vec![false; belongs_to.foreign_key.fields.len()],
        operands: vec![],
        fail: false,
    };

    lift.visit_filter(&select.filter);

    // Fall back to the IN-subquery form whenever we couldn't account for
    // every FK column with a direct equality from the filter. This covers
    // both `fail=true` (a binary op referenced something that isn't on the
    // FK) and the case where the filter contained no liftable binary ops at
    // all — e.g. when it's a nested `InSubquery` that the LiftBelongsTo
    // visitor deliberately skips (see `visit_expr_in_subquery`).
    let all_fks_matched = lift.fk_field_matches.iter().all(|m| *m);

    if lift.fail || !all_fks_matched {
        let mut subquery = query.clone();

        subquery.body.as_select_mut_unwrap().returning = stmt::Returning::Project(
            super::key_field_refs(0, belongs_to.foreign_key.fields.iter().map(|fk| fk.target)),
        );

        Some(stmt::Expr::in_subquery(
            super::key_field_refs(0, belongs_to.foreign_key.fields.iter().map(|fk| fk.source)),
            subquery,
        ))
    } else {
        Some(if lift.operands.len() == 1 {
            lift.operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd {
                operands: lift.operands,
            }
            .into()
        })
    }
}

/// HasOne/HasMany branch: rewrite to a foreign-key IN subquery against
/// the related table. Single-column FKs produce a scalar IN; composite
/// FKs produce a tuple-form IN that the SQL serializer renders as
/// `(a, b) IN (SELECT a, b FROM ...)`.
fn lift_has_n_in_subquery(
    target: ModelId,
    pair: &BelongsTo,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    if target != query.body.as_select_unwrap().source.model_id_unwrap() {
        return None;
    }

    let mut subquery = query.clone();

    match &mut subquery.body {
        stmt::ExprSet::Select(select) => {
            select.returning = stmt::Returning::Project(super::key_field_refs(
                0,
                pair.foreign_key.fields.iter().map(|fk| fk.source),
            ));
        }
        _ => todo!(),
    }

    Some(
        stmt::ExprInSubquery {
            expr: Box::new(super::key_field_refs(
                0,
                pair.foreign_key.fields.iter().map(|fk| fk.target),
            )),
            query: Box::new(subquery),
        }
        .into(),
    )
}

impl Visit for LiftBelongsTo<'_> {
    fn visit_expr_in_subquery(&mut self, _i: &stmt::ExprInSubquery) {
        // Stop the walk at a nested IN-subquery boundary. Field references
        // inside resolve in the nested query's own scope, but the visitor's
        // `cx` is scoped to the current (outer) subquery's source — recursing
        // would misresolve inner refs as outer-model fields and produce
        // bogus FK matches. The main LiftInSubquery walker handles the
        // nested subquery on its own pass, with the correct scope.
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        match (&*i.lhs, &*i.rhs) {
            (stmt::Expr::Reference(expr_reference), other)
            | (other, stmt::Expr::Reference(expr_reference)) => {
                assert!(i.op.is_eq() || i.op.is_ne());

                if i.op.is_eq() || i.op.is_ne() {
                    let field = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .as_field_unwrap();

                    self.lift_fk_constraint(field.id, i.op, other);
                } else {
                    self.fail = true;
                }
            }
            // Constraints we can't lift to a direct FK comparison (e.g. a
            // projection through an embedded field).  Bail to the IN-subquery
            // form so the filter is preserved verbatim; without this, the
            // empty `operands` list silently produced an empty AND (= true)
            // and the subquery returned every row.
            _ => {
                self.fail = true;
            }
        }
    }
}

impl LiftBelongsTo<'_> {
    fn lift_fk_constraint(&mut self, field: FieldId, op: stmt::BinaryOp, expr: &stmt::Expr) {
        for (i, fk_field) in self.belongs_to.foreign_key.fields.iter().enumerate() {
            if fk_field.target == field {
                if self.fk_field_matches[i] {
                    todo!("not handled");
                }

                self.operands.push(stmt::Expr::binary_op(
                    stmt::Expr::ref_self_field(fk_field.source),
                    op,
                    expr.clone(),
                ));
                self.fk_field_matches[i] = true;

                return;
            }
        }

        self.fail = true;
    }
}
