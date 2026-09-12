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
//!   share [`lift_relation_path_predicate`], as does the variant check
//!   ([`try_lift_relation_path_is_variant`]) on an enum reached through a
//!   relation (`link.item.state IS Selected`).
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

use super::embedded_relation::{
    EmbeddedRelationRef, Step, build_expr_path, flatten_expr_path, resolve_embedded_relation,
};

/// Pre-lowering pass that lifts relation references out of `IN`-subquery
/// and projection comparisons into direct foreign-key forms.  Runs as a
/// whole-statement visitor before the main lowering walk: code paths that
/// process expressions outside the lowering walk (notably
/// `ApplyInsertScope::apply_expr`) see the already-lifted form.
pub(super) struct LiftInSubquery<'a> {
    cx: ExprContext<'a>,
    exclude_nulls: bool,
}

impl<'a> LiftInSubquery<'a> {
    pub(super) fn new(cx: ExprContext<'a>, exclude_nulls: bool) -> Self {
        Self { cx, exclude_nulls }
    }

    pub(super) fn rewrite(&mut self, stmt: &mut stmt::Statement) {
        self.visit_mut(stmt);
    }

    fn scope<'scope>(&'scope self, target: impl IntoExprTarget<'scope>) -> LiftInSubquery<'scope> {
        LiftInSubquery {
            cx: self.cx.scope(target),
            exclude_nulls: self.exclude_nulls,
        }
    }

    /// Try the embedded-relation rewrites on one expression, replacing it
    /// when a rewrite fires.
    fn rewrite_embedded_relation(&self, expr: &mut stmt::Expr) {
        let lifted = match expr {
            stmt::Expr::BinaryOp(e) => {
                if rewrite_embedded_relation_operand(&self.cx, e) {
                    return;
                }
                lift_embedded_relation_comparison(&self.cx, e)
            }
            stmt::Expr::Like(e) => lift_embedded_relation_like(&self.cx, e),
            stmt::Expr::IsVariant(e) => lift_embedded_relation_is_variant(&self.cx, e),
            stmt::Expr::InSubquery(e) => lift_embedded_relation_in_subquery(&self.cx, e),
            stmt::Expr::InList(e) => {
                rewrite_embedded_relation_in_list(&self.cx, e);
                None
            }
            _ => None,
        };

        if let Some(mut lifted) = lifted {
            self.exclude_nulls(&mut lifted);
            *expr = lifted;
        }
    }

    fn exclude_nulls(&self, expr: &mut stmt::Expr) {
        if !self.exclude_nulls {
            return;
        }

        let stmt::Expr::InSubquery(expr) = expr else {
            return;
        };
        let select = expr.query.body.as_select_mut_unwrap();
        let target = select.source.model_id_unwrap();
        let returning = select.returning.as_project_unwrap().clone();

        for field in returning
            .as_record()
            .map_or(std::slice::from_ref(&returning), |record| &record.fields)
        {
            let stmt::Expr::Reference(stmt::ExprReference::Field { index, .. }) = field else {
                unreachable!();
            };
            if self.cx.schema().app.field(target.field(*index)).nullable {
                select.add_filter(stmt::Expr::is_not_null(field.clone()));
            }
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
                if let Some(mut lifted) = lift_in_subquery(&self.cx, &e.expr, &e.query) {
                    self.exclude_nulls(&mut lifted);
                    *expr = lifted;
                } else {
                    self.rewrite_embedded_relation(expr);
                }
            }
            stmt::Expr::BinaryOp(e) => {
                if let Some(mut lifted) =
                    try_lift_relation_path_comparison(&self.cx, e.op, &e.lhs, &e.rhs)
                {
                    self.exclude_nulls(&mut lifted);
                    *expr = lifted;
                } else if let Some(commuted) = e.op.commute()
                    && let Some(mut lifted) =
                        try_lift_relation_path_comparison(&self.cx, commuted, &e.rhs, &e.lhs)
                {
                    self.exclude_nulls(&mut lifted);
                    *expr = lifted;
                } else {
                    self.rewrite_embedded_relation(expr);
                }
            }
            stmt::Expr::Like(e) => {
                if let Some(mut lifted) = try_lift_relation_path_like(&self.cx, e) {
                    self.exclude_nulls(&mut lifted);
                    *expr = lifted;
                } else {
                    self.rewrite_embedded_relation(expr);
                }
            }
            // A variant check whose subject walks a relation is a predicate
            // on the target model, like a comparison: the guard the typed
            // layer fixed next to `link.item.state.selected.value == x`
            // lifts into its own foreign-key subquery.
            stmt::Expr::IsVariant(e) => {
                if let Some(mut lifted) = try_lift_relation_path_is_variant(&self.cx, e) {
                    self.exclude_nulls(&mut lifted);
                    *expr = lifted;
                } else {
                    self.rewrite_embedded_relation(expr);
                }
            }
            stmt::Expr::InList(_) => self.rewrite_embedded_relation(expr),
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
        stmt::Expr::Project(_) | stmt::Expr::Variant(_) => {
            return lift_projection_in_subquery(cx, expr, query);
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
    path: &stmt::Expr,
    query: &stmt::Query,
) -> Option<stmt::Expr> {
    let (expr_ref, steps) = flatten_expr_path(path)?;
    let ResolvedRef::Field(field) = cx.resolve_expr_reference(&expr_ref) else {
        return None;
    };

    let target_model_id = field.relation_target_id()?;

    let (Step::Field(head_idx), tail) = steps.split_first()? else {
        return None;
    };

    if tail.is_empty()
        && let Some(direct) =
            try_fuse_paired_relations(cx, field, target_model_id, *head_idx, query)
    {
        return Some(direct);
    }

    let inner_lhs = reroot(target_model_id, *head_idx, tail);

    let new_subquery = stmt::Query::new_select(
        stmt::Source::from(target_model_id),
        Expr::in_subquery(inner_lhs, query.clone()),
    );

    lift_in_subquery(cx, &Expr::Reference(expr_ref), &new_subquery)
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

/// Rewrites a variant check whose subject walks a relation field into a
/// foreign-key subquery — [`try_lift_relation_path_comparison`] for the
/// `is_variant` guards of a predicate through a relation.
///
/// `Link::filter(Link::fields().item().state().is_selected())` rewrites to:
///
/// ```text
/// Link.item_id IN (SELECT Item.id FROM Item WHERE Item.state IS Selected)
/// ```
///
/// Returns `None` when the subject does not walk a relation field.
pub(super) fn try_lift_relation_path_is_variant(
    cx: &ExprContext,
    is_variant: &stmt::ExprIsVariant,
) -> Option<stmt::Expr> {
    lift_relation_path_predicate(cx, &is_variant.expr, |target_lhs| {
        Expr::is_variant(target_lhs, is_variant.variant)
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
    if !matches!(project_side, Expr::Project(_) | Expr::Variant(_)) {
        return None;
    }
    let (expr_ref, steps) = flatten_expr_path(project_side)?;
    let ResolvedRef::Field(field) = cx.resolve_expr_reference(&expr_ref) else {
        return None;
    };

    let target_model_id = field.relation_target_id()?;

    // The first step names the relation field on the source model; the
    // rest index into the target model. Drop the first step and re-root the
    // remainder at the target model.
    let (Step::Field(head_idx), tail) = steps.split_first()? else {
        return None;
    };
    let target_lhs = reroot(target_model_id, *head_idx, tail);

    let subquery =
        stmt::Query::new_select(stmt::Source::from(target_model_id), make_filter(target_lhs));

    lift_in_subquery(cx, &Expr::Reference(expr_ref), &subquery)
}

/// Re-root a path at `model`: the field at `head` followed by `tail`. Variant
/// selections in `tail` are preserved, so a path into an enum variant of the
/// target model stays a selection there.
fn reroot(model: ModelId, head: usize, tail: &[Step]) -> Expr {
    build_expr_path(Expr::ref_self_field(FieldId { model, index: head }), tail)
}

/// `<relation-path> eq/ne <expr>`: substitute the projection of the
/// relation's key field(s) for the relation reference, in place. The
/// analogue of `rewrite_eq_operand`'s `BelongsTo` arm for relations inside
/// embedded types. The key expression keeps the path's variant selections,
/// so the `is_variant` guards the typed layer fixed next to the comparison
/// still scope it (see `resolve_embedded_relation`).
///
/// Returns `true` when an operand was substituted; the binary op itself
/// stays in place. Both operands are checked — comparing one embedded
/// relation to another (key-versus-key) substitutes both sides; stopping
/// at the first would leave the other side lowering to its storage-less
/// `Null` slot.
fn rewrite_embedded_relation_operand(cx: &ExprContext, e: &mut stmt::ExprBinaryOp) -> bool {
    if !e.op.is_eq() && !e.op.is_ne() {
        return false;
    }

    let mut rewrote = false;

    for side in [&mut e.lhs, &mut e.rhs] {
        if let Some(resolved) = resolve_embedded_relation(cx, side)
            && resolved.tail.is_empty()
        {
            **side = resolved.key_expr;
            rewrote = true;
        }
    }

    rewrote
}

/// `<relation-path> IN (list)`: substitute the projection of the relation's
/// key field(s) for the relation reference, in place — the
/// [`rewrite_embedded_relation_operand`] case for list membership. The list
/// holds model values, which the typed layer already reduced to their keys.
fn rewrite_embedded_relation_in_list(cx: &ExprContext, e: &mut stmt::ExprInList) {
    if let Some(resolved) = resolve_embedded_relation(cx, &e.expr)
        && resolved.tail.is_empty()
    {
        *e.expr = resolved.key_expr;
    }
}

/// A comparison whose one side projects *through* an embedded relation into
/// the target model (`v.human().name().eq("Alice")`) lifts to a foreign-key
/// `IN` subquery on the target — [`lift_relation_path_predicate`] for
/// relations inside embedded types.
fn lift_embedded_relation_comparison(
    cx: &ExprContext,
    e: &stmt::ExprBinaryOp,
) -> Option<stmt::Expr> {
    let sides = [
        (&e.lhs, &e.rhs, Some(e.op)),
        (&e.rhs, &e.lhs, e.op.commute()),
    ];

    for (project_side, other_side, op) in sides {
        let Some(resolved) = resolve_embedded_relation(cx, project_side) else {
            continue;
        };
        // A comparison *at* the relation is the operand rewrite's case.
        if resolved.tail.is_empty() {
            continue;
        }

        let filter = Expr::binary_op(reroot_tail(&resolved), op?, (**other_side).clone());
        let query = stmt::Query::new_select(stmt::Source::from(resolved.belongs_to.target), filter);
        return embedded_fk_in_subquery(&resolved, query);
    }

    None
}

/// [`lift_embedded_relation_comparison`] for `LIKE` / `ILIKE` patterns —
/// the counterpart of [`try_lift_relation_path_like`].
fn lift_embedded_relation_like(cx: &ExprContext, like: &stmt::ExprLike) -> Option<stmt::Expr> {
    lift_embedded_relation_predicate(cx, &like.expr, |target_lhs| {
        stmt::ExprLike {
            expr: Box::new(target_lhs),
            pattern: like.pattern.clone(),
            escape: like.escape,
            case_insensitive: like.case_insensitive,
        }
        .into()
    })
}

/// [`lift_embedded_relation_comparison`] for a variant check on an enum of
/// the target model (`v.human().state().is_selected()`) — the counterpart of
/// [`try_lift_relation_path_is_variant`].
fn lift_embedded_relation_is_variant(
    cx: &ExprContext,
    is_variant: &stmt::ExprIsVariant,
) -> Option<stmt::Expr> {
    lift_embedded_relation_predicate(cx, &is_variant.expr, |target_lhs| {
        Expr::is_variant(target_lhs, is_variant.variant)
    })
}

/// Shared core of the predicate lifts through an embedded relation:
/// `subject` walks the relation into its target model, and `make_filter`
/// builds the target-model predicate from the re-rooted subject.
fn lift_embedded_relation_predicate(
    cx: &ExprContext,
    subject: &stmt::Expr,
    make_filter: impl FnOnce(stmt::Expr) -> stmt::Expr,
) -> Option<stmt::Expr> {
    let resolved = resolve_embedded_relation(cx, subject)?;
    if resolved.tail.is_empty() {
        return None;
    }

    let filter = make_filter(reroot_tail(&resolved));
    let query = stmt::Query::new_select(stmt::Source::from(resolved.belongs_to.target), filter);
    embedded_fk_in_subquery(&resolved, query)
}

/// `<relation-path> IN (subquery)` for an embedded relation:
/// `v.human().in_query(..)` when the path ends at the relation, and relation
/// chains continuing past it (re-rooted and recursed, as in
/// [`lift_projection_in_subquery`]).
fn lift_embedded_relation_in_subquery(
    cx: &ExprContext,
    e: &stmt::ExprInSubquery,
) -> Option<stmt::Expr> {
    let resolved = resolve_embedded_relation(cx, &e.expr)?;

    if resolved.tail.is_empty() {
        embedded_fk_in_subquery(&resolved, (*e.query).clone())
    } else {
        let inner = Expr::in_subquery(reroot_tail(&resolved), (*e.query).clone());
        let query = stmt::Query::new_select(stmt::Source::from(resolved.belongs_to.target), inner);
        embedded_fk_in_subquery(&resolved, query)
    }
}

/// Re-root the path steps continuing past an embedded relation at the
/// relation's target model.
fn reroot_tail(resolved: &EmbeddedRelationRef<'_>) -> Expr {
    let (Step::Field(head), rest) = resolved.tail.split_first().expect("tail must be non-empty")
    else {
        panic!("a relation's target model is not an enum")
    };
    reroot(resolved.belongs_to.target, *head, rest)
}

/// Build the foreign-key `IN` subquery for an embedded relation edge: the
/// projected key field(s) on the host side, the FK target fields as the
/// subquery's returning. Returns `None` when the subquery does not target
/// the relation's target model.
fn embedded_fk_in_subquery(
    resolved: &EmbeddedRelationRef<'_>,
    mut query: stmt::Query,
) -> Option<stmt::Expr> {
    if resolved.belongs_to.target != query.body.as_select_unwrap().source.model_id_unwrap() {
        return None;
    }

    query.body.as_select_mut_unwrap().returning = stmt::Returning::Project(super::key_field_refs(
        0,
        resolved
            .belongs_to
            .foreign_key
            .fields
            .iter()
            .map(|fk| fk.target),
    ));

    Some(stmt::Expr::in_subquery(resolved.key_expr.clone(), query))
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
