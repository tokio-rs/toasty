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
//!   one side walks a relation field. For example, filtering profiles by
//!   their user's name (`profile.user.name = 'alice'`) rewrites to:
//!
//!   ```text
//!   Profile.user_id IN (SELECT User.id FROM User WHERE User.name = 'alice')
//!   ```
//!
//! - [`try_lift_relation_path_like`] does the same for `LIKE`/`ILIKE`
//!   (`profile.user.name LIKE 'al%'`). These are `Expr::Like`, not
//!   `Expr::BinaryOp`, so they need their own entry point. Both rewrites
//!   share [`lift_relation_path_predicate`].
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
    schema::app::{self, BelongsTo, FieldId, FieldTy, ModelId},
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
                } else if let Some(commuted) = e.op.commute()
                    && let Some(lifted) =
                        try_lift_relation_path_comparison(&self.cx, commuted, &e.rhs, &e.lhs)
                {
                    *expr = lifted;
                }
            }
            stmt::Expr::Like(e) => {
                if let Some(lifted) = try_lift_relation_path_like(&self.cx, e) {
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
        // `Project(Ref(rel), [head, ...tail])` — the path traverses through a
        // relation field (`rel`) before reaching the relation the subquery
        // targets. Re-root the path at `rel`'s target model and rebuild as a
        // nested IN-subquery on that model, then recurse to lift the outer
        // `rel` hop.
        stmt::Expr::Project(project_expr) => {
            return lift_projection_in_subquery(cx, project_expr, query);
        }
        stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Field { .. }) => {
            cx.resolve_expr_reference(expr_reference).as_field_unwrap()
        }
        _ => {
            return None;
        }
    };

    // If the field is not a relation, abort. Direct relations lift through
    // their paired foreign keys. A `via` has no single pair, so normalize its
    // relation chain into a projected path and use the projection lift.
    match &field.ty {
        FieldTy::BelongsTo(belongs_to) => lift_belongs_to_in_subquery(cx, belongs_to, query),
        FieldTy::Has(has) => lift_has_n_in_subquery(has.target, has.pair(&cx.schema().app), query),
        FieldTy::Via(via) => lift_via_in_subquery(cx, via, query),
        _ => None,
    }
}

/// Lift an `IN` subquery whose left side names a `via` relation.
///
/// For `User.groups.any(Group.name == "Rust")`, where `groups` follows
/// `User.memberships.group`, this builds:
///
/// ```text
/// User.id IN (
///     SELECT Membership.user_id FROM Membership
///     WHERE Membership.group_id IN (
///         SELECT Group.id FROM Group WHERE Group.name == "Rust"
///     )
/// )
/// ```
///
/// The path is expanded into direct relation fields, converted to the same
/// projected expression produced by an explicit relation chain, and then
/// handled by the regular projection lift.
fn lift_via_in_subquery(
    cx: &ExprContext,
    via: &app::Via,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    if via.is_scalar() {
        return None;
    }

    let fields = super::relation_path::flatten_via_path(cx.schema(), via)?;
    let target = fields
        .last()
        .and_then(|field| cx.schema().app.field(*field).relation_target_id())?;

    if target != via.target || target != query.body.as_select_unwrap().source.model_id_unwrap() {
        return None;
    }

    let (base, projection) = fields.split_first()?;
    let base = stmt::Expr::ref_self_field(*base);
    let path = if projection.is_empty() {
        base
    } else {
        let projection = projection
            .iter()
            .map(|field| field.index)
            .collect::<Vec<_>>();
        stmt::Expr::project(base, projection.as_slice())
    };

    lift_in_subquery(cx, &path, query)
}

/// Lifts an `IN`-subquery whose left side is a path through a relation field.
///
/// `.any()` on a relation chain — for example, `Release.project.topics.any(name
/// == "rust")` — builds an `InSubquery` whose LHS projects through the
/// chain:
///
/// ```text
/// InSubquery {
///     expr:  Project(Ref(Release.project), [Project.topics_idx]),
///     query: SELECT FROM Topic WHERE name == "rust",
/// }
/// ```
///
/// Two paths handle this:
///
/// **Fused.** [`try_fuse_paired_relations`] recognizes the common
/// `BelongsTo → Has` chain over a shared primary key and emits a single
/// FK-on-FK `IN` that bypasses the intermediate model:
///
/// ```text
/// Release.project_id IN (SELECT Topic.project_id FROM Topic WHERE name == "rust")
/// ```
///
/// **General.** For chains the fast path doesn't match (multi-hop
/// projections, non-paired relations), re-root the projection at the
/// relation's target model and wrap the original subquery in a nested `IN`
/// against that model, then recurse on the outer hop. For the same input,
/// the fallback produces:
///
/// ```text
/// Release.project IN (
///     SELECT FROM Project
///     WHERE Project.topics IN (SELECT FROM Topic WHERE name == "rust")
/// )
/// ```
///
/// The recursive call lifts the outer `Release.project` hop via the standard
/// `BelongsTo` branch; the inner `Project.topics` hop is lifted on the next
/// visitor pass when [`LiftInSubquery`]'s children walk reaches it.
fn lift_projection_in_subquery(
    cx: &ExprContext,
    project_expr: &stmt::ExprProject,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    let Expr::Reference(expr_ref) = &*project_expr.base else {
        return None;
    };
    let ResolvedRef::Field(field) = cx.resolve_expr_reference(expr_ref) else {
        return None;
    };

    let target_model_id = field.relation_target_id()?;

    let (head_idx, tail) = project_expr.projection.as_slice().split_first()?;

    if tail.is_empty()
        && let Some(direct) =
            try_fuse_paired_relations(cx, field, target_model_id, *head_idx, query)
    {
        return Some(direct);
    }

    let target_field = Expr::ref_self_field(FieldId {
        model: target_model_id,
        index: *head_idx,
    });
    let inner_lhs = if tail.is_empty() {
        target_field
    } else {
        Expr::project(target_field, stmt::Projection::from(tail))
    };

    let new_subquery = stmt::Query::new_select(
        stmt::Source::from(target_model_id),
        Expr::in_subquery(inner_lhs, query.clone()),
    );

    lift_in_subquery(cx, &project_expr.base, &new_subquery)
}

/// Fuses a `BelongsTo → Has` chain into a single FK-on-FK `IN` when both
/// relations meet at the same primary key.
///
/// # Example
///
/// Given the schema:
///
/// ```text
/// Todo     { category_id: Category.id, ... }   // BelongsTo  Todo.category
/// Category { todos: HasMany Todo, ... }        // Has, paired with Todo.category
/// ```
///
/// The path `Todo.category.todos` lifts as follows:
///
/// ```text
/// // Input
/// InSubquery {
///     expr:  Project(Ref(Todo.category), [Category.todos_idx]),
///     query: SELECT FROM Todo WHERE title == "salad",
/// }
///
/// // Output
/// InSubquery {
///     expr:  Ref(Todo.category_id),
///     query: SELECT Todo.category_id FROM Todo WHERE title == "salad",
/// }
/// ```
///
/// The outer relation's FK source columns become the LHS; the inner
/// relation's paired-BelongsTo FK source columns become the subquery's
/// returning list. The user's original filter is preserved verbatim.
///
/// # Why the fusion is sound
///
/// Composing the two hops without fusion routes through the intermediate
/// model:
///
/// ```text
/// Todo.category_id IN (
///     SELECT Category.id FROM Category
///     WHERE Category.id IN (SELECT Todo.category_id FROM Todo WHERE title == "salad")
/// )
/// ```
///
/// Both FKs target `Category.id`, so every value the innermost subquery
/// returns exists in `Category.id` under FK integrity (which the engine
/// already assumes). The middle filter therefore admits every row the inner
/// returns, and the middle `SELECT Category.id` simply re-emits them. The
/// `Category` scan is a no-op and can be dropped.
///
/// Returns `None` when the chain doesn't match the pattern — outer isn't a
/// `BelongsTo`, inner isn't a `Has`, or the FK columns don't line up.
fn try_fuse_paired_relations(
    cx: &ExprContext,
    outer_field: &app::Field,
    target_model_id: ModelId,
    head_idx: usize,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    let outer_belongs_to = match &outer_field.ty {
        FieldTy::BelongsTo(rel) => rel,
        _ => return None,
    };

    let target_model = cx.schema().app.model(target_model_id).as_root_unwrap();
    let head_field = target_model.fields.get(head_idx)?;
    let inner_has = match &head_field.ty {
        FieldTy::Has(has) => has,
        _ => return None,
    };
    let inner_pair = inner_has.pair(&cx.schema().app);

    // Both FKs must reference the same PK columns in the same order.
    if outer_belongs_to.foreign_key.fields.len() != inner_pair.foreign_key.fields.len() {
        return None;
    }
    for (outer_fk, inner_fk) in outer_belongs_to
        .foreign_key
        .fields
        .iter()
        .zip(inner_pair.foreign_key.fields.iter())
    {
        if outer_fk.target != inner_fk.target {
            return None;
        }
    }

    lift_fk_in_subquery(
        inner_has.target,
        super::key_field_refs(
            0,
            outer_belongs_to
                .foreign_key
                .fields
                .iter()
                .map(|fk| fk.source),
        ),
        super::key_field_refs(0, inner_pair.foreign_key.fields.iter().map(|fk| fk.source)),
        query,
    )
}

/// Rewrites a comparison that walks a relation field into a foreign-key
/// subquery.
///
/// `Profile::filter(Profile::fields().user().name().eq("alice"))` starts as
/// the filter `profile.user.name = 'alice'`, where the left side projects
/// through the `user` relation. This moves the comparison into a subquery on
/// the target model and defers to [`lift_in_subquery`]:
///
/// ```text
/// Profile.user_id IN (SELECT User.id FROM User WHERE User.name = 'alice')
/// ```
///
/// Returns `None` when `project_side` does not walk a relation field.
pub(super) fn try_lift_relation_path_comparison(
    cx: &ExprContext,
    op: stmt::BinaryOp,
    project_side: &stmt::Expr,
    other_side: &stmt::Expr,
) -> Option<stmt::Expr> {
    lift_relation_path_predicate(cx, project_side, |target_lhs| {
        Expr::binary_op(target_lhs, op, other_side.clone())
    })
}

/// Rewrites a `LIKE`/`ILIKE` that walks a relation field into a foreign-key
/// subquery — [`try_lift_relation_path_comparison`] for pattern matches.
///
/// `Profile::filter(Profile::fields().user().name().like("al%"))` rewrites to:
///
/// ```text
/// Profile.user_id IN (SELECT User.id FROM User WHERE User.name LIKE 'al%')
/// ```
///
/// `LIKE`/`ILIKE` are `Expr::Like`, not `Expr::BinaryOp`, so they never reach
/// [`try_lift_relation_path_comparison`] and need this entry point. Without
/// it the `user.name` path stays a projection through the relation, which the
/// rest of lowering cannot turn into a column and so panics.
///
/// Returns `None` when the pattern's subject does not walk a relation field.
pub(super) fn try_lift_relation_path_like(
    cx: &ExprContext,
    like: &stmt::ExprLike,
) -> Option<stmt::Expr> {
    lift_relation_path_predicate(cx, &like.expr, |target_lhs| {
        stmt::ExprLike {
            expr: Box::new(target_lhs),
            pattern: like.pattern.clone(),
            escape: like.escape,
            case_insensitive: like.case_insensitive,
        }
        .into()
    })
}

/// Shared core of the relation-path lifts.
///
/// `project_side` is the side of the predicate that walks a relation — the
/// `profile.user.name` in `profile.user.name = 'alice'`. It is a projection
/// whose first step names the `user` relation field on `Profile` and whose
/// remaining steps index into `User`. This re-roots the path at the target
/// model (so it reads `User.name`), builds a `SELECT` over `User` whose filter
/// `make_filter` produces from the re-rooted path, and defers to
/// [`lift_in_subquery`] to turn the relation reference into the foreign-key
/// `IN` form.
///
/// `make_filter` is the only difference between callers: a binary op for
/// comparisons, a `LIKE` for pattern matches.
///
/// Returns `None` when `project_side` does not walk a relation field.
fn lift_relation_path_predicate(
    cx: &ExprContext,
    project_side: &stmt::Expr,
    make_filter: impl FnOnce(stmt::Expr) -> stmt::Expr,
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

    let target_model_id = field.relation_target_id()?;

    // The first step names the relation field on the source model; the
    // rest index into the target model. Drop the first step and re-root the
    // remainder at the target model.
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

    let subquery =
        stmt::Query::new_select(stmt::Source::from(target_model_id), make_filter(target_lhs));

    lift_in_subquery(cx, &project_expr.base, &subquery)
}

/// Build a foreign-key `IN` subquery for one direct relation edge.
///
/// `lhs` references key fields on the current model. `returning` references
/// the matching key fields on `target`, which is also the subquery source.
fn lift_fk_in_subquery(
    target: ModelId,
    lhs: stmt::Expr,
    returning: stmt::Expr,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    if target != query.body.as_select_unwrap().source.model_id_unwrap() {
        return None;
    }

    let mut subquery = query.clone();
    subquery.body.as_select_mut_unwrap().returning = stmt::Returning::Project(returning);

    Some(stmt::Expr::in_subquery(lhs, subquery))
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
        lift_fk_in_subquery(
            belongs_to.target,
            super::key_field_refs(0, belongs_to.foreign_key.fields.iter().map(|fk| fk.source)),
            super::key_field_refs(0, belongs_to.foreign_key.fields.iter().map(|fk| fk.target)),
            query,
        )
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
    lift_fk_in_subquery(
        target,
        super::key_field_refs(0, pair.foreign_key.fields.iter().map(|fk| fk.target)),
        super::key_field_refs(0, pair.foreign_key.fields.iter().map(|fk| fk.source)),
        query,
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
