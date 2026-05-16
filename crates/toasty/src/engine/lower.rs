mod association;
mod expr_or;
mod expr_pattern;
mod include;
mod insert;
mod lift_in_subquery;
mod paginate;
mod relation;
mod returning;

#[cfg(test)]
mod tests;

use std::cell::Cell;

use hashbrown::HashSet;

use index_vec::IndexVec;
use toasty_core::{
    Result, Schema,
    driver::Capability,
    schema::{
        app::{self, ModelRoot},
        db::ColumnId,
        mapping,
    },
    stmt::{self, IntoExprTarget, VisitMut, visit_mut},
};

use crate::engine::{Engine, HirStatement, fold, hir, simplify::Simplify};

impl Engine {
    pub(super) fn lower_stmt(&self, stmt: stmt::Statement) -> Result<HirStatement> {
        let schema = &self.schema;

        let mut state = LoweringState {
            hir: HirStatement::new(),
            scopes: IndexVec::new(),
            engine: self,
            relations: vec![],
            errors: vec![],
            dependencies: HashSet::new(),
        };

        state.lower_stmt(stmt::ExprContext::new(schema), None, stmt);

        if let Some(err) = state.errors.into_iter().next() {
            return Err(err);
        }

        Ok(state.hir)
    }
}

impl LoweringState<'_> {
    fn lower_stmt(
        &mut self,
        expr_cx: stmt::ExprContext,
        row_index: Option<usize>,
        mut stmt: stmt::Statement,
    ) -> hir::StmtId {
        // App-level rewrites that lowering depends on. `Source::Model { via }`
        // must be converted to a WHERE filter before the lowering walk
        // converts `Source::Model` into `Source::Table`.  The IN-subquery
        // lift fires here too: code paths outside the lowering walk (e.g.
        // `ApplyInsertScope::apply_expr`) see the already-lifted form.
        // The eq/ne operand rewrite (model→PK, BelongsTo→FK) fires inside
        // the lowering walk itself via `LowerStatement::visit_expr_binary_op_mut`.
        association::RewriteVia::new(expr_cx).rewrite(&mut stmt);
        lift_in_subquery::LiftInSubquery::new(expr_cx).rewrite(&mut stmt);

        Simplify::with_context(expr_cx, self.engine.capability).visit_mut(&mut stmt);

        let stmt_id = self.hir.new_statement_info(self.dependencies.clone());
        let scope_id = self.scopes.push(Scope { stmt_id, row_index });
        let mut collect_dependencies = None;

        // Map the statement
        LowerStatement {
            state: self,
            expr_cx,
            scope_id,
            cx: LoweringContext::Statement,
            collect_dependencies: &mut collect_dependencies,
        }
        .visit_stmt_mut(&mut stmt);

        self.engine.simplify_stmt(&mut stmt);

        let stmt_info = &mut self.hir[stmt_id];
        stmt_info.stmt = Some(Box::new(stmt));

        self.scopes.pop();

        debug_assert!(collect_dependencies.is_none());

        stmt_id
    }
}

struct LowerStatement<'a, 'b> {
    /// Lowering state. This is the state that is constant throughout the entire
    /// lowering process.
    state: &'a mut LoweringState<'b>,

    /// Expression context in which the statement is being lowered
    expr_cx: stmt::ExprContext<'a>,

    /// Identifier to the current scope (stored in `scopes` on LoweringState)
    scope_id: ScopeId,

    /// Current lowering context
    cx: LoweringContext<'a>,

    /// Track dependencies here.
    collect_dependencies: &'a mut Option<HashSet<hir::StmtId>>,
}

#[derive(Debug)]
struct LoweringState<'a> {
    /// Database engine handle
    engine: &'a Engine,

    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    hir: HirStatement,

    /// Scope state
    scopes: IndexVec<ScopeId, Scope>,

    /// Planning a query can require walking relations to maintain data
    /// consistency. This field tracks the current relation edge being traversed
    /// so the planner doesn't walk it backwards.
    relations: Vec<app::FieldId>,

    /// All new statements should include these as part of its dependencies
    dependencies: HashSet<hir::StmtId>,

    /// Tracks errors that occurred while lowering the statement
    errors: Vec<crate::Error>,
}

#[derive(Debug, Clone, Copy)]
enum LoweringContext<'a> {
    /// Lowering an insertion statement
    Insert(&'a [ColumnId], Option<usize>),

    /// Lowering a value row being inserted
    InsertRow(&'a stmt::Expr),

    /// Lowering the returning clause of a statement. Optionally carries the
    /// parent INSERT's row index when visiting a per-row returning expression.
    Returning(Option<usize>),

    /// All other lowering cases
    Statement,
}

#[derive(Debug)]
struct Scope {
    /// Identifier of the statement in the lowering state
    stmt_id: hir::StmtId,

    /// If the statement is called from an insert's values (i.e. the parent
    /// statement is an insert), this tracks the row index
    row_index: Option<usize>,
}

index_vec::define_index_type! {
    struct ScopeId = u32;
}

/// A collection operator on a `Vec<scalar>` field.
///
/// `Append`, `Remove`, `Pop`, and `RemoveAt` are all collection operators:
/// they target a single-column `Vec<scalar>` field and carry operator
/// operands (a list of elements, a single element, an index, or nothing).
/// Whether the operand is a list or a scalar is a per-operator encoding
/// detail — the unifying property is that none carries a *field value*
/// that decomposes across columns, so none needs the `model_to_table`
/// substitution that `Set` flows through.
enum CollectionOp<'a> {
    Append(&'a mut stmt::Expr),
    Remove(&'a mut stmt::Expr),
    Pop,
    RemoveAt(&'a mut stmt::Expr),
}

impl LowerStatement<'_, '_> {
    /// Lower a `Set` assignment — the only assignment that carries a whole
    /// *field value*.
    ///
    /// The field may span multiple columns (embedded structs decompose
    /// across columns; enums into a discriminant plus variant columns), so
    /// this resolves the field mapping and, for each impacted column,
    /// substitutes the lowered value into that column's `model_to_table`
    /// template before emitting the column-level write.
    fn lower_set_assignment(
        &mut self,
        out: &mut stmt::Assignments,
        mapping: &toasty_core::schema::mapping::Model,
        projection: &stmt::Projection,
        expr: &mut stmt::Expr,
    ) {
        self.visit_expr_mut(expr);

        let Some(field) = mapping.resolve_field_mapping(projection) else {
            self.state
                .errors
                .push(crate::Error::invalid_statement(format!(
                    "invalid assignment projection: {projection:?}"
                )));
            return;
        };

        for (column, lowering_idx) in field.columns() {
            let mut lowering_expr = mapping.model_to_table[lowering_idx].clone();

            // Substitute the field reference in the column's lowering
            // template with the lowered value.
            lowering_expr.substitute(AssignmentInput {
                assignment_projection: projection.clone(),
                value: expr,
            });

            self.visit_expr_mut(&mut lowering_expr);

            out.set(column, lowering_expr);
        }
    }

    /// Lower a collection operator (`Append` / `Remove` / `Pop` /
    /// `RemoveAt`) on a `Vec<scalar>` field.
    ///
    /// `Vec<scalar>` fields always resolve to a single primitive column, so
    /// this skips the `model_to_table` substitution that `lower_set_assignment`
    /// performs — the operands are operator arguments, not a field value,
    /// and for a single-column field `model_to_table` is identity anyway.
    ///
    /// `Append` is supported on every backend; the removal operators are
    /// gated by per-backend capability flags and emit a clear error where
    /// the native form is not available.
    fn lower_collection_op(
        &mut self,
        out: &mut stmt::Assignments,
        mapping: &toasty_core::schema::mapping::Model,
        projection: &stmt::Projection,
        op: CollectionOp,
    ) {
        let Some(field) = mapping.resolve_field_mapping(projection) else {
            self.state
                .errors
                .push(crate::Error::invalid_statement(format!(
                    "invalid assignment projection: {projection:?}"
                )));
            return;
        };

        let Some(prim) = field.as_primitive() else {
            self.state
                .errors
                .push(crate::Error::invalid_statement(format!(
                    "collection operator on non-primitive field: {projection:?}"
                )));
            return;
        };

        // `Append` is universally supported; the removal operators are
        // gated per backend.
        let cap = self.capability();
        let unsupported = match &op {
            CollectionOp::Append(_) => None,
            CollectionOp::Remove(_) if !cap.vec_remove => Some("stmt::remove"),
            CollectionOp::Pop if !cap.vec_pop => Some("stmt::pop"),
            CollectionOp::RemoveAt(_) if !cap.vec_remove_at => Some("stmt::remove_at"),
            _ => None,
        };

        if let Some(op_name) = unsupported {
            self.state
                .errors
                .push(crate::Error::invalid_statement(format!(
                    "{op_name} is not yet supported on this backend"
                )));
            return;
        }

        match op {
            CollectionOp::Append(expr) => {
                self.visit_expr_mut(expr);
                out.append(prim.column, expr.take());
            }
            CollectionOp::Remove(expr) => {
                self.visit_expr_mut(expr);
                out.remove(prim.column, expr.take());
            }
            CollectionOp::Pop => {
                out.pop(prim.column);
            }
            CollectionOp::RemoveAt(expr) => {
                self.visit_expr_mut(expr);
                out.remove_at(prim.column, expr.take());
            }
        }
    }
}

impl LowerStatement<'_, '_> {
    fn new_dependency(&mut self, stmt: impl Into<stmt::Statement>) -> hir::StmtId {
        let row_index = match self.cx {
            LoweringContext::Insert(_, row_index) => row_index,
            LoweringContext::Returning(row_index) => row_index,
            _ => None,
        };

        let stmt_id = self.state.lower_stmt(self.expr_cx, row_index, stmt.into());

        if let Some(dependencies) = &mut self.collect_dependencies {
            dependencies.insert(stmt_id);
        }

        self.curr_stmt_info().deps.insert(stmt_id);

        stmt_id
    }

    fn collect_dependencies(
        &mut self,
        f: impl FnOnce(&mut LowerStatement<'_, '_>),
    ) -> HashSet<hir::StmtId> {
        let old = self.collect_dependencies.replace(HashSet::new());
        f(self);
        std::mem::replace(self.collect_dependencies, old).unwrap()
    }

    fn track_dependency(&mut self, dependency: hir::StmtId) {
        self.curr_stmt_info().deps.insert(dependency);
    }

    fn with_dependencies(
        &mut self,
        mut dependencies: HashSet<hir::StmtId>,
        f: impl FnOnce(&mut LowerStatement<'_, '_>),
    ) {
        // Dependencies should stack
        dependencies.extend(&self.state.dependencies);

        let old = std::mem::replace(&mut self.state.dependencies, dependencies);
        f(self);
        self.state.dependencies = old;
    }
}

impl visit_mut::VisitMut for LowerStatement<'_, '_> {
    fn visit_order_by_expr_mut(&mut self, node: &mut stmt::OrderByExpr) {
        // First, run the default visitor to lower sub-expressions
        self.visit_expr_mut(&mut node.expr);

        // Reuse binary-op lowering: synthesize `expr == expr` so that
        // cast conversions are applied, then keep the LHS result.
        let mut lhs = node.expr.clone();
        let mut rhs = node.expr.take();
        self.lower_expr_binary_op(stmt::BinaryOp::Eq, &mut lhs, &mut rhs);
        node.expr = lhs;
    }

    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut lowered = stmt::Assignments::default();
        let mapping = self.mapping_unwrap();

        // Two lowering paths, divided by what the assignment carries:
        //
        // - `Set` carries a whole *field value*. It is the only assignment
        //   that can target a multi-column field (embedded structs decompose
        //   across columns, enums into a discriminant plus variants), so it
        //   is the only one that needs the `model_to_table` substitution.
        //
        // - `Append` / `Remove` / `Pop` / `RemoveAt` are *collection
        //   operators*. They carry operator operands (a list, an element,
        //   an index, or nothing) and always target a single-column
        //   `Vec<scalar>` field, so none needs `model_to_table`.
        for (projection, assignment) in &mut *i {
            match assignment {
                stmt::Assignment::Set(expr) => {
                    self.lower_set_assignment(&mut lowered, mapping, projection, expr);
                }
                stmt::Assignment::Append(expr) => {
                    self.lower_collection_op(
                        &mut lowered,
                        mapping,
                        projection,
                        CollectionOp::Append(expr),
                    );
                }
                stmt::Assignment::Remove(expr) => {
                    self.lower_collection_op(
                        &mut lowered,
                        mapping,
                        projection,
                        CollectionOp::Remove(expr),
                    );
                }
                stmt::Assignment::Pop => {
                    self.lower_collection_op(&mut lowered, mapping, projection, CollectionOp::Pop);
                }
                stmt::Assignment::RemoveAt(expr) => {
                    self.lower_collection_op(
                        &mut lowered,
                        mapping,
                        projection,
                        CollectionOp::RemoveAt(expr),
                    );
                }
                stmt::Assignment::Insert(_) | stmt::Assignment::Batch(_) => {
                    todo!(
                        "Insert / Batch assignments are not produced for table lowering; got {assignment:#?}"
                    )
                }
            }
        }

        *i = lowered;
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut stmt::ExprSetOp) {
        todo!("stmt={i:#?}");
    }

    fn visit_expr_binary_op_mut(&mut self, i: &mut stmt::ExprBinaryOp) {
        // App-level operand rewrite for `eq` / `ne` (`Reference::Model` →
        // primary-key field, `BelongsTo` → foreign-key field).  Must fire
        // before the operands are walked, since walking lowers the field
        // references into column references and the rewrite has nothing to
        // match on.  Nested binary ops are handled by the recursive walk
        // through `visit_expr_mut`.
        if i.op.is_eq() || i.op.is_ne() {
            self.rewrite_eq_operand(&mut i.lhs);
            self.rewrite_eq_operand(&mut i.rhs);
        }

        stmt::visit_mut::visit_expr_binary_op_mut(self, i);
    }

    fn visit_expr_in_list_mut(&mut self, i: &mut stmt::ExprInList) {
        // App-level operand rewrite: `Reference::Model { nesting } IN list`
        // becomes `<pk_field> IN list`.  Must fire before the LHS is walked,
        // since walking lowers the model reference into something the rewrite
        // can no longer match on.
        self.rewrite_in_list_model_operand(i);

        stmt::visit_mut::visit_expr_in_list_mut(self, i);
    }

    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::BinaryOp(e) => {
                self.visit_expr_binary_op_mut(e);

                if let Some(lowered) = self.lower_expr_binary_op(e.op, &mut e.lhs, &mut e.rhs) {
                    *expr = lowered;
                }
            }
            // App-level rewrite: an OR covering every variant of an enum
            // via `IsVariant` is a tautology.  Must fire before the children
            // walk lowers the `IsVariant` nodes into discriminant
            // comparisons, since the rewrite pattern-matches on
            // `Expr::IsVariant`.
            stmt::Expr::Or(e) if expr_or::is_variant_tautology_or(self.expr_cx.schema(), e) => {
                *expr = true.into();
            }
            stmt::Expr::InList(e) => {
                self.visit_expr_in_list_mut(e);

                if let Some(lowered) = self.lower_expr_in_list(&mut e.expr, &mut e.list) {
                    *expr = lowered;
                }

                // PostgreSQL-style: `x IN (a, b, c)` → `x = ANY($1)` with the
                // list bound as a single array param.
                if let stmt::Expr::InList(e) = expr
                    && self.supports_any_rewrite()
                    && in_list_is_value_list(e)
                {
                    let stmt::Expr::InList(e) = expr.take() else {
                        unreachable!()
                    };
                    *expr = stmt::Expr::any_op(*e.expr, stmt::BinaryOp::Eq, *e.list);
                }
            }
            stmt::Expr::Not(e) if matches!(*e.expr, stmt::Expr::InList(_)) => {
                // Recurse into inner first so the InList itself is lowered.
                self.visit_expr_not_mut(e);

                // If the inner is still an InList (not yet rewritten because
                // the gate is off or the list shape doesn't qualify), leave
                // `Not(InList)` alone and let the SQL serializer render
                // `NOT IN`. Otherwise, rewrite `NOT (x = ANY(arr))` into the
                // canonical `x <> ALL(arr)` form.
                if self.supports_any_rewrite()
                    && let stmt::Expr::AnyOp(any) = e.expr.as_mut()
                    && any.op == stmt::BinaryOp::Eq
                {
                    let stmt::Expr::Not(not) = expr.take() else {
                        unreachable!()
                    };
                    let stmt::Expr::AnyOp(any) = *not.expr else {
                        unreachable!()
                    };
                    *expr = stmt::Expr::all_op(*any.lhs, stmt::BinaryOp::Ne, *any.rhs);
                }
            }
            stmt::Expr::InSubquery(e) => {
                if self.capability().sql {
                    self.visit_expr_in_subquery_mut(e);

                    let maybe_res = self.lower_expr_binary_op(
                        stmt::BinaryOp::Eq,
                        &mut e.expr,
                        e.query.returning_mut_unwrap().as_project_mut_unwrap(),
                    );

                    assert!(maybe_res.is_none(), "TODO");

                    let returning = e.query.returning_mut_unwrap().as_project_mut_unwrap();

                    if !returning.is_record() {
                        *returning = stmt::Expr::record([returning.take()]);
                    }
                } else {
                    self.visit_expr_mut(&mut e.expr);

                    let source_id = self.scope_stmt_id();
                    let target_id = self.scope_statement(|child| {
                        child.visit_stmt_query_mut(&mut e.query);
                    });

                    // For now, we wonly support independent sub-queries. I.e.
                    // the subquery must be able to be executed without any
                    // context from the parent query.
                    let target_stmt_info = &self.state.hir[target_id];
                    debug_assert!(target_stmt_info.args.is_empty(), "TODO");
                    debug_assert!(target_stmt_info.back_refs.is_empty(), "TODO");

                    self.track_dependency(target_id);

                    let maybe_res = self.lower_expr_binary_op(
                        stmt::BinaryOp::Eq,
                        &mut e.expr,
                        e.query.returning_mut_unwrap().as_project_mut_unwrap(),
                    );

                    assert!(maybe_res.is_none(), "TODO");

                    let stmt::Expr::InSubquery(e) = expr.take() else {
                        panic!()
                    };

                    let arg =
                        self.new_sub_statement(source_id, target_id, Box::new((*e.query).into()));

                    *expr = stmt::ExprInList {
                        expr: e.expr,
                        list: Box::new(arg),
                    }
                    .into();
                }
            }
            stmt::Expr::IsVariant(e) => {
                // Look up the enum model and variant directly via VariantId
                let enum_model = self
                    .schema()
                    .app
                    .model(e.variant.model)
                    .as_embedded_enum_unwrap();
                let has_data = enum_model.has_data_variants();
                let disc_value = enum_model.variants[e.variant.index].discriminant.clone();

                // Lower the inner expression
                self.visit_expr_mut(&mut e.expr);

                let lowered_expr = e.expr.take();

                // Emit the appropriate comparison
                if has_data {
                    // Data-carrying: project([0]) to extract discriminant from Record
                    *expr = stmt::Expr::eq(
                        stmt::Expr::project(lowered_expr, [0usize]),
                        stmt::Expr::Value(disc_value),
                    );
                } else {
                    // Unit-only: compare directly
                    *expr = stmt::Expr::eq(lowered_expr, stmt::Expr::Value(disc_value));
                }
            }
            stmt::Expr::Reference(expr_reference) => {
                match expr_reference {
                    // A reference to a relation field inside a Returning
                    // clause becomes a subquery that loads the related
                    // model(s).  This is the `.select(rel_field)` path; it
                    // mirrors the include-subquery machinery that
                    // `.include(...)` uses for `Returning::Model`.
                    stmt::ExprReference::Field { nesting: 0, index }
                        if matches!(self.cx, LoweringContext::Returning(_))
                            && self.model_unwrap().fields[*index].ty.is_relation() =>
                    {
                        *expr = self.build_relation_subquery(*index, &[]);
                    }
                    stmt::ExprReference::Field { nesting, index } => {
                        *expr = self.lower_expr_field(*nesting, *index);
                        self.visit_expr_mut(expr);
                    }
                    stmt::ExprReference::Model { .. } => todo!(),
                    stmt::ExprReference::Column(expr_column) => {
                        if expr_column.nesting > 0 {
                            let source_id = self.scope_stmt_id();
                            let target_id = self.resolve_stmt_id(expr_column.nesting);

                            // the current scope ID should also be the top of the stack
                            debug_assert_eq!(self.state.scopes.len(), self.scope_id + 1);

                            // The statement is not independent. Walk up the
                            // scope stack until the referened target statement
                            // and flag any intermediate statements as also not
                            // indepdnendent.
                            for scope in self.state.scopes.iter().rev() {
                                if scope.stmt_id == target_id {
                                    break;
                                }

                                self.state.hir[scope.stmt_id].independent = false;
                            }

                            let position = self.new_ref(source_id, target_id, *expr_reference);

                            // Using ExprArg as a placeholder. It will be rewritten
                            // later.
                            *expr = stmt::Expr::arg(position);
                        }
                    }
                }
            }
            stmt::Expr::Stmt(_) => {
                let stmt::Expr::Stmt(mut expr_stmt) = expr.take() else {
                    panic!()
                };

                // Expr::Stmt subqueries are valid in returning expressions (e.g.,
                // INCLUDE preloading) and in VALUES bodies of batch queries.
                debug_assert!(
                    self.cx.is_returning() || matches!(self.cx, LoweringContext::Statement),
                    "cx={:#?}",
                    self.cx,
                );

                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.
                let source_id = self.scope_stmt_id();
                let target_id = self.scope_statement(|child| {
                    visit_mut::visit_expr_stmt_mut(child, &mut expr_stmt);
                });

                // Cheap canonicalization is enough here: the parent statement's
                // post-lowering simplify will recursively visit this embedded
                // sub-statement and apply the heavyweight rules.
                fold::fold_stmt(&mut *expr_stmt.stmt);

                *expr = self.new_sub_statement(source_id, target_id, expr_stmt.stmt);

                if self.state.hir[target_id].independent {
                    self.curr_stmt_info().deps.insert(target_id);
                }
            }
            stmt::Expr::StartsWith(_)
                if self.capability().sql && !self.capability().native_starts_with =>
            {
                self.lower_expr_starts_with(expr);
            }
            stmt::Expr::Exists(_) if !self.capability().sql => {
                let stmt::Expr::Exists(mut expr_exists) = expr.take() else {
                    panic!()
                };

                // Extract the EXISTS subquery into a sub-statement so the
                // executor can evaluate the condition in memory.
                let source_id = self.scope_stmt_id();
                let target_id = self.scope_statement(|child| {
                    child.visit_stmt_query_mut(&mut expr_exists.subquery);
                });

                let mut stmt = stmt::Statement::Query(*expr_exists.subquery);
                // Cheap canonicalization is enough here: the parent statement's
                // post-lowering simplify will recursively visit this embedded
                // sub-statement and apply the heavyweight rules.
                fold::fold_stmt(&mut stmt);

                let arg = self.new_sub_statement(source_id, target_id, Box::new(stmt));

                if self.state.hir[target_id].independent {
                    self.curr_stmt_info().deps.insert(target_id);
                }

                // The sub-statement result is a list of rows. Wrap it in
                // a single-row VALUES query so the evaluator unwraps the
                // outer list, letting EXISTS check the inner row count.
                let mut subquery = stmt::Query::values(arg);
                subquery.single = true;
                *expr = stmt::Expr::Exists(stmt::ExprExists {
                    subquery: Box::new(subquery),
                });
            }
            _ => {
                // Recurse down the statement tree
                stmt::visit_mut::visit_expr_mut(self, expr);
            }
        }
    }

    fn visit_insert_target_mut(&mut self, i: &mut stmt::InsertTarget) {
        match i {
            stmt::InsertTarget::Scope(_) => todo!("stmt={i:#?}"),
            stmt::InsertTarget::Model(model_id) => {
                let mapping = self.schema().mapping_for(model_id);
                *i = stmt::InsertTable {
                    table: mapping.table,
                    columns: mapping.columns.clone(),
                }
                .into();
            }
            _ => todo!(),
        }
    }

    fn visit_update_target_mut(&mut self, i: &mut stmt::UpdateTarget) {
        match i {
            stmt::UpdateTarget::Query(_) => todo!("update_target={i:#?}"),
            stmt::UpdateTarget::Model(model_id) => {
                let table_id = self.schema().table_id_for(model_id);
                *i = stmt::UpdateTarget::table(table_id);
            }
            stmt::UpdateTarget::Table(_) => {}
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        // Delegate to the default visitor which dispatches to
        // visit_stmt_insert_mut, visit_stmt_query_mut, etc.
        stmt::visit_mut::visit_expr_stmt_mut(self, i);
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        if let stmt::Returning::Model { include } = i {
            // Start from the schema's pre-computed default returning — every
            // `#[deferred]` field, top-level or nested, is already `Null`.
            // `process_top_level_includes` then splices loaded forms in for
            // the fields named by include paths (and for every deferred field
            // when this is an `INSERT ... RETURNING`).
            let mut returning = self.mapping_unwrap().default_returning.clone();
            let include_paths = std::mem::take(include);
            let is_insert = self.cx.is_insert();

            self.process_top_level_includes(&mut returning, &include_paths, is_insert);

            *i = stmt::Returning::Project(returning);
        }

        // For multi-row INSERT returning, visit each row with its row index so
        // that sub-statements (e.g., child INSERTs for HasOne relations) capture
        // the correct parent row index via scope_statement.
        if matches!(&self.cx, LoweringContext::Insert(..))
            && let stmt::Returning::Expr(stmt::Expr::List(list)) = i
        {
            for (index, item) in list.items.iter_mut().enumerate() {
                self.lower_returning_for_row(index).visit_expr_mut(item);
            }
            return;
        }

        stmt::visit_mut::visit_returning_mut(&mut self.lower_returning(), i);
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        // Create a new expr scope for the statement, and lower all parts
        // *except* the source field (since it is borrowed).
        let mut lower = self.scope_expr(&stmt.from);

        // Before lowering, handle cascading deletes
        lower.plan_stmt_delete_relations(stmt);

        lower.visit_filter_mut(&mut stmt.filter);

        if let Some(expr) = &mut stmt.condition.expr {
            lower.visit_expr_mut(expr);
        }

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
        }

        lower.apply_lowering_filter_constraint(&mut stmt.filter);

        self.visit_source_mut(&mut stmt.from);
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        // First, if an insertion scope is specified, lower the scope to be just "model"
        self.apply_insert_scope(&mut stmt.target, &mut stmt.source);

        // Create a new expr scope for the statement, and lower all parts
        // *except* the target field (since it is borrowed).
        let mut lower = self.lower_insert(&stmt.target);

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
        }

        // Preprocess the insertion source (values usually)
        lower.preprocess_insert_values(&mut stmt.source, &mut stmt.returning);

        // Lower the insertion source
        lower.visit_stmt_query_mut(&mut stmt.source);

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
            lower.constantize_insert_returning(returning, &stmt.source);

            if stmt.source.single
                && let stmt::Returning::Expr(expr) = &returning
            {
                // Not strictly true, but there is nothing that needs to
                // return a list at this point for a "single" query. If this
                // is ever needed, remove the assertion.
                debug_assert!(!expr.is_list());
            }
        }

        self.visit_insert_target_mut(&mut stmt.target);
    }

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        let mut lower = self.scope_expr(&stmt.body);

        if let Some(with) = &mut stmt.with {
            lower.visit_with_mut(with);
        }

        if let Some(order_by) = &mut stmt.order_by {
            lower.visit_order_by_mut(order_by);
        }

        if let Some(limit) = &mut stmt.limit {
            lower.visit_limit_mut(limit);
        }

        self.visit_expr_set_mut(&mut stmt.body);

        self.rewrite_offset_after_as_filter(stmt);
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        let mut lower = self.scope_expr(&stmt.source);

        lower.visit_filter_mut(&mut stmt.filter);
        lower.visit_returning_mut(&mut stmt.returning);
        lower.apply_lowering_filter_constraint(&mut stmt.filter);

        self.visit_source_mut(&mut stmt.source);
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        let mut lower = self.scope_expr(&stmt.target);

        let mut returning_changed = false;

        // Before lowering children, convert the "Changed" returning statement
        // to an expression referencing changed fields.
        if let Some(returning) = &mut stmt.returning
            && returning.is_changed()
        {
            returning_changed = true;

            if let Some(model) = lower.model() {
                let mapping = lower.mapping_unwrap();

                // Step 1 — build a mask of all primitives being changed by
                // OR-ing each assigned field's coverage mask together.
                let mut changed_bits = stmt::PathFieldSet::new();
                for projection in stmt.assignments.keys() {
                    if let Some(mf) = mapping.resolve_field_mapping(projection) {
                        changed_bits |= mf.field_mask();
                    }
                }

                // Step 2 — build the returning expression.
                *returning = stmt::Returning::Project(build_update_returning(
                    model.id,
                    None,
                    &mapping.fields,
                    &changed_bits,
                ));
            }
        }

        // Plan relations
        lower.plan_stmt_update_relations(
            &mut stmt.assignments,
            &stmt.filter,
            &mut stmt.returning,
            returning_changed,
        );

        lower.visit_assignments_mut(&mut stmt.assignments);
        lower.visit_filter_mut(&mut stmt.filter);

        if let Some(expr) = &mut stmt.condition.expr {
            lower.visit_expr_mut(expr);
        }

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
            // Use the lowered assignments (which are now column-indexed)
            lower.constantize_update_returning(returning, &stmt.assignments);
        }

        self.visit_update_target_mut(&mut stmt.target);
    }

    fn visit_source_mut(&mut self, stmt: &mut stmt::Source) {
        if let stmt::Source::Model(source_model) = stmt {
            debug_assert!(source_model.via.is_none(), "TODO");

            let table_id = self.schema().table_id_for(source_model.id);
            *stmt = stmt::Source::table(table_id);
        }
    }

    fn visit_values_mut(&mut self, stmt: &mut stmt::Values) {
        if self.cx.is_insert()
            && let Some(mapping) = self.mapping()
        {
            for row in &mut stmt.rows {
                let mut lowered = mapping.model_to_table.clone();
                self.lower_insert_row(row)
                    .visit_expr_record_mut(&mut lowered);

                *row = lowered.into();
            }

            return;
        }

        visit_mut::visit_values_mut(self, stmt);
    }
}

impl<'a, 'b> LowerStatement<'a, 'b> {
    /// App-level operand rewrite for `eq` and `ne` binary ops.
    ///
    /// `Reference::Model { nesting }` becomes a reference to the model's
    /// primary-key field; a `BelongsTo` field reference becomes a reference
    /// to the relation's foreign-key field.  Both rewrites only match on
    /// app-level shapes; after the surrounding lowering walk converts the
    /// references to columns, this method has nothing to rewrite.
    ///
    /// Must fire before the operand's children are visited, since lowering
    /// otherwise replaces the field reference with a column reference and
    /// the rewrite has no app-level shape to match on.
    fn rewrite_eq_operand(&self, operand: &mut stmt::Expr) {
        if let stmt::Expr::Reference(expr_reference) = operand {
            match &*expr_reference {
                stmt::ExprReference::Model { nesting } => {
                    let model = self
                        .expr_cx
                        .resolve_expr_reference(expr_reference)
                        .as_model_unwrap();

                    let [pk_field] = &model.primary_key.fields[..] else {
                        todo!("handle composite keys");
                    };

                    *operand = stmt::Expr::ref_field(*nesting, pk_field);
                }
                stmt::ExprReference::Field { .. } => {
                    let field = self
                        .expr_cx
                        .resolve_expr_reference(expr_reference)
                        .as_field_unwrap();

                    match &field.ty {
                        app::FieldTy::Primitive(_) | app::FieldTy::Embedded(_) => {}
                        app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => todo!(),
                        app::FieldTy::BelongsTo(rel) => {
                            let [fk_field] = &rel.foreign_key.fields[..] else {
                                todo!("handle composite keys");
                            };

                            let stmt::ExprReference::Field { index, .. } = expr_reference else {
                                panic!()
                            };
                            *index = fk_field.source.index;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// App-level rewrite for the LHS of an `IN`-list expression:
    /// `Reference::Model { nesting } IN list` becomes `<pk_field> IN list`.
    ///
    /// Must fire before the LHS is walked, since walking lowers the model
    /// reference into a column reference and the rewrite has nothing to
    /// match on.
    fn rewrite_in_list_model_operand(&self, expr: &mut stmt::ExprInList) {
        let (nesting, pk_field_id) = {
            let stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Model { nesting }) =
                &*expr.expr
            else {
                return;
            };
            let nesting = *nesting;
            let model = self
                .expr_cx
                .resolve_expr_reference(expr_ref)
                .as_model_unwrap();
            let [pk_field_id] = &model.primary_key.fields[..] else {
                todo!()
            };
            (nesting, *pk_field_id)
        };

        let pk = self.expr_cx.schema().app.field(pk_field_id);

        // Sanity-check the RHS shape against the PK type.
        match &mut *expr.list {
            stmt::Expr::List(expr_list) => {
                for item in &mut expr_list.items {
                    match item {
                        stmt::Expr::Value(value) => {
                            assert!(value.is_a(&pk.ty.as_primitive_unwrap().ty));
                        }
                        _ => todo!("{item:#?}"),
                    }
                }
            }
            stmt::Expr::Value(stmt::Value::List(values)) => {
                for value in values {
                    assert!(value.is_a(&pk.ty.as_primitive_unwrap().ty));
                }
            }
            _ => todo!("expr={expr:#?}"),
        }

        *expr.expr = stmt::Expr::ref_field(nesting, pk.id());
    }

    fn lower_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::Value(value), other) | (other, stmt::Expr::Value(value))
                if value.is_null() =>
            {
                let other = other.take();

                Some(match op {
                    stmt::BinaryOp::Eq => stmt::Expr::is_null(other),
                    stmt::BinaryOp::Ne => stmt::Expr::is_not_null(other),
                    _ => todo!(),
                })
            }
            // Record-vs-record decomposition for eq/ne. Embedded fields lower
            // to record expressions, so a comparison like
            // `Record([cast(col, T)]) == Record([val])` only exposes the
            // cast (and the cast-stripping rule below) once the record is
            // split per-element. Recurse into each pair so cast handling
            // fires.
            //
            // Mirrors the two record shapes handled in `simplify_expr_binary_op`:
            // both sides `Expr::Record`, or one side folded to
            // `Expr::Value(Value::Record)`.
            (stmt::Expr::Record(lhs_rec), stmt::Expr::Record(rhs_rec))
                if (op.is_eq() || op.is_ne()) && lhs_rec.len() == rhs_rec.len() =>
            {
                Some(self.combine_record_op(
                    op,
                    std::mem::take(&mut lhs_rec.fields),
                    std::mem::take(&mut rhs_rec.fields),
                ))
            }
            (stmt::Expr::Record(rec), stmt::Expr::Value(stmt::Value::Record(val_rec)))
            | (stmt::Expr::Value(stmt::Value::Record(val_rec)), stmt::Expr::Record(rec))
                if (op.is_eq() || op.is_ne()) && rec.len() == val_rec.len() =>
            {
                let val_exprs = std::mem::take(&mut val_rec.fields)
                    .into_iter()
                    .map(stmt::Expr::Value)
                    .collect();
                Some(self.combine_record_op(op, std::mem::take(&mut rec.fields), val_exprs))
            }
            (stmt::Expr::Cast(expr_cast), _) | (_, stmt::Expr::Cast(expr_cast)) => {
                let target_ty = self.capability().native_type_for(&expr_cast.ty);
                self.cast_expr(lhs, &target_ty);
                self.cast_expr(rhs, &target_ty);
                None
            }
            _ => None,
        }
    }

    /// Combines per-element comparisons of two record-shaped operands into a
    /// single boolean expression: AND for `eq`, OR for `ne`. Each pair is
    /// recursed through `lower_expr_binary_op` so any inner cast handling
    /// fires as if the comparison had been written elementwise.
    fn combine_record_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs_fields: Vec<stmt::Expr>,
        rhs_fields: Vec<stmt::Expr>,
    ) -> stmt::Expr {
        let comparisons: Vec<_> = lhs_fields
            .into_iter()
            .zip(rhs_fields)
            .map(|(mut l, mut r)| {
                self.lower_expr_binary_op(op, &mut l, &mut r)
                    .unwrap_or_else(|| stmt::Expr::binary_op(l, op, r))
            })
            .collect();

        if op.is_eq() {
            stmt::Expr::and_from_vec(comparisons)
        } else {
            stmt::Expr::or_from_vec(comparisons)
        }
    }

    fn lower_expr_in_list(
        &mut self,
        expr: &mut stmt::Expr,
        list: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *expr, list) {
            (expr, stmt::Expr::Map(expr_map)) => {
                assert!(expr_map.base.is_arg(), "TODO");
                let maybe_res =
                    self.lower_expr_binary_op(stmt::BinaryOp::Eq, expr, &mut expr_map.map);

                assert!(maybe_res.is_none(), "TODO");
                None
            }
            (stmt::Expr::Cast(expr_cast), list) => {
                let target_ty = self.capability().native_type_for(&expr_cast.ty);
                self.cast_expr(expr, &target_ty);

                match list {
                    stmt::Expr::List(expr_list) => {
                        for item in &mut expr_list.items {
                            self.cast_expr(item, &target_ty);
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(items)) => {
                        for item in items {
                            *item = target_ty.cast(item.take()).expect("failed to cast value");
                        }
                    }
                    stmt::Expr::Arg(_) => {
                        let arg = list.take();
                        let cast = stmt::Expr::cast(stmt::Expr::arg(0), target_ty);
                        *list = stmt::Expr::map(arg, cast);
                    }
                    _ => todo!("expr={expr:#?}; list={list:#?}"),
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::List(list)) => {
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                for item in &mut list.items {
                    assert!(item.is_value());
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::Value(stmt::Value::List(_))) => {
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                None
            }
            (stmt::Expr::Reference(expr_reference), list) => {
                assert!(expr_reference.is_column());

                match list {
                    stmt::Expr::Value(stmt::Value::List(_)) => {}
                    stmt::Expr::List(list) => {
                        for item in &list.items {
                            assert!(item.is_value());
                        }
                    }
                    _ => panic!("invalid; should have been caught earlier"),
                }

                None
            }
            (expr, list) => todo!("expr={expr:#?}; list={list:#?}"),
        }
    }

    fn apply_lowering_filter_constraint(&self, _filter: &mut stmt::Filter) {}

    fn lower_expr_field(&self, nesting: usize, index: usize) -> stmt::Expr {
        match self.cx {
            LoweringContext::Statement | LoweringContext::Returning(_) => {
                let mapping = self.mapping_at_unwrap(nesting);
                mapping.table_to_model.lower_expr_reference(nesting, index)
            }
            LoweringContext::InsertRow(row) => {
                // If nesting > 0, this references a parent scope, not the current row
                if nesting > 0 {
                    // Use Statement context to properly handle cross-statement references
                    let mapping = self.mapping_at_unwrap(nesting);
                    mapping.table_to_model.lower_expr_reference(nesting, index)
                } else {
                    row.entry(index).unwrap().to_expr()
                }
            }
            _ => todo!("cx={:#?}", self.cx),
        }
    }

    /// Returns the ArgId for the new reference
    fn new_ref(
        &mut self,
        source_id: hir::StmtId,
        target_id: hir::StmtId,
        mut expr_reference: stmt::ExprReference,
    ) -> usize {
        let stmt::ExprReference::Column(expr_column) = &mut expr_reference else {
            todo!()
        };

        // First, get the nesting so we can resolve the target statmeent
        let nesting = expr_column.nesting;

        // We only track references that point to statements being executed by
        // separate operations. References within the same operation are handled
        // by the target database.
        debug_assert!(nesting != 0, "expr_reference={expr_reference:#?}");

        // Set the nesting to zero as the stored ExprReference will be used from
        // the context of the *target* statement.
        expr_column.nesting = 0;

        let target = &mut self.state.hir[target_id];

        // The `batch_load_index` is the index for this reference in the row
        // returned from the target statement's ExecStatement operation. This
        // ExecStatement operation batch loads all records needed to execute
        // the full root statement.
        target
            .back_refs
            .entry(source_id)
            .or_default()
            .exprs
            .insert_full(expr_reference);

        // Create an argument for inputing the expr reference's value into the statement.
        let source = &mut self.state.hir[source_id];

        // See if an arg already exists
        for (i, arg) in source.args.iter().enumerate() {
            let hir::Arg::Ref {
                target_expr_ref, ..
            } = arg
            else {
                continue;
            };

            if *target_expr_ref == expr_reference {
                return i;
            }
        }

        let arg = source.args.len();

        source.args.push(hir::Arg::Ref {
            target_expr_ref: expr_reference,
            stmt_id: target_id,
            nesting,
            data_load_input: Cell::new(None),
            returning_input: Cell::new(None),
            batch_load_index: if let Some(row_index) = self.state.scopes[self.scope_id].row_index {
                debug_assert_eq!(1, nesting, "TODO");
                Cell::new(Some(row_index))
            } else {
                Cell::new(None)
            },
        });

        arg
    }

    fn new_statement_info(&mut self) -> hir::StmtId {
        let mut deps = self.state.dependencies.clone();
        deps.extend(&self.curr_stmt_info().deps);

        self.state.hir.new_statement_info(deps)
    }

    /// Create a new sub-statement. Returns the argument position
    fn new_sub_statement(
        &mut self,
        source_id: hir::StmtId,
        target_id: hir::StmtId,
        stmt: Box<stmt::Statement>,
    ) -> stmt::Expr {
        self.state.hir[target_id].stmt = Some(stmt);
        self.new_dependency_arg(source_id, target_id)
    }

    /// Create a new argument on a dependent statement
    fn new_dependency_arg(&mut self, source_id: hir::StmtId, target_id: hir::StmtId) -> stmt::Expr {
        let source = &mut self.state.hir[source_id];
        let arg = source.args.len();
        source.args.push(hir::Arg::Sub {
            stmt_id: target_id,
            returning: self.cx.is_returning(),
            input: Cell::new(None),
            batch_load_index: Cell::new(None),
        });

        stmt::Expr::arg(arg)
    }

    /// Run the canonical pipeline (pre-lower simplify, lowering walk, post-lower
    /// simplify) on a synthesized sub-statement and stitch it onto the parent
    /// as an `Expr::Arg`.
    ///
    /// Used at sites where lowering itself synthesizes a new statement to embed
    /// in the parent's `Returning` (include subqueries, child inserts for
    /// relation planning).  Equivalent to a recursive `lower_stmt` call that
    /// passes through the `Expr::Stmt` arm in `visit_expr_mut`, but expressed
    /// directly so the synthesized statement does not have to round-trip
    /// through an `Expr::Stmt` placeholder.
    fn lower_sub_stmt(&mut self, stmt: stmt::Statement) -> stmt::Expr {
        let source_id = self.scope_stmt_id();
        let mut stmt = Box::new(stmt);

        let target_id = self.scope_statement(|child| {
            // Via-association rewrite: `Source::Model { via }` becomes an
            // explicit WHERE filter so the lowering walk only sees rewritten
            // sources.  The IN-subquery lift fires next so non-walk code
            // paths see the lifted form.  The eq/ne operand rewrite
            // (model→PK, BelongsTo→FK) fires inside the lowering walk via
            // `LowerStatement::visit_expr_binary_op_mut`.
            association::RewriteVia::new(child.expr_cx).rewrite(&mut stmt);
            lift_in_subquery::LiftInSubquery::new(child.expr_cx).rewrite(&mut stmt);
            // Pre-lower simplify: remaining heavyweight rules the lowering
            // visitor expects to have already fired.
            Simplify::with_context(child.expr_cx, child.state.engine.capability)
                .visit_mut(&mut *stmt);
            // Lowering walk.
            child.visit_stmt_mut(&mut stmt);
            // Post-lower simplify: heavyweight rules on the lowered tree.
            child.state.engine.simplify_stmt(&mut *stmt);
        });

        // Sub-statements built via this helper always live in the parent's
        // Returning clause (include subqueries, child inserts for relation
        // planning).  Force the new `Arg::Sub`'s `returning` flag accordingly:
        // `plan_nested_merge` keys off `returning: true` to discover include
        // subqueries, and the line-399 `Expr::Stmt` arm gets the same flag
        // because it fires during the parent's Returning walk.
        let saved_cx = std::mem::replace(&mut self.cx, LoweringContext::Returning(None));
        let arg = self.new_sub_statement(source_id, target_id, stmt);
        self.cx = saved_cx;

        if self.state.hir[target_id].independent {
            self.curr_stmt_info().deps.insert(target_id);
        }

        arg
    }

    fn schema(&self) -> &'b Schema {
        &self.state.engine.schema
    }

    fn capability(&self) -> &Capability {
        self.state.engine.capability()
    }

    /// Both flags must be true to rewrite `IN (...)` into `= ANY(<array>)`:
    /// the dialect must accept the predicate, and the bind layer must accept
    /// a single array-valued parameter.
    fn supports_any_rewrite(&self) -> bool {
        let cap = self.capability();
        cap.bind_list_param && cap.predicate_match_any
    }

    fn field(&self, id: impl Into<app::FieldId>) -> &'b app::Field {
        self.schema().app.field(id.into())
    }

    fn model(&self) -> Option<&'a ModelRoot> {
        self.expr_cx.target().as_model()
    }

    #[track_caller]
    fn model_unwrap(&self) -> &'a ModelRoot {
        self.expr_cx.target().as_model_unwrap()
    }

    fn mapping(&self) -> Option<&'b mapping::Model> {
        self.model()
            .map(|model| self.state.engine.schema.mapping_for(model))
    }

    #[track_caller]
    fn mapping_unwrap(&self) -> &'b mapping::Model {
        self.state.engine.schema.mapping_for(self.model_unwrap())
    }

    #[track_caller]
    fn mapping_at_unwrap(&self, nesting: usize) -> &'b mapping::Model {
        let model = self.expr_cx.target_at(nesting).as_model_unwrap();
        self.state.engine.schema.mapping_for(model)
    }

    fn curr_stmt_info(&mut self) -> &mut hir::StatementInfo {
        let stmt_id = self.scope_stmt_id();
        &mut self.state.hir[stmt_id]
    }

    /// Returns the `StmtId` for the Statement at the **current** scope.
    fn scope_stmt_id(&self) -> hir::StmtId {
        self.state.scopes[self.scope_id].stmt_id
    }

    /// Get the StmtId for the specified nesting level
    fn resolve_stmt_id(&self, nesting: usize) -> hir::StmtId {
        debug_assert!(
            self.scope_id >= nesting,
            "invalid nesting; nesting={nesting:#?}; scopes={:#?}",
            self.state.scopes
        );
        self.state.scopes[self.scope_id - nesting].stmt_id
    }

    /// Plan a sub-statement that is able to reference the parent statement
    fn scope_statement(&mut self, f: impl FnOnce(&mut LowerStatement<'_, '_>)) -> hir::StmtId {
        let stmt_id = self.new_statement_info();
        let row_index = match &self.cx {
            LoweringContext::Insert(_, row_index) => *row_index,
            LoweringContext::Returning(row_index) => *row_index,
            _ => None,
        };
        let scope_id = self.state.scopes.push(Scope { stmt_id, row_index });
        let mut dependencies = None;

        let mut lower = LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id,
            cx: LoweringContext::Statement,
            collect_dependencies: &mut dependencies,
        };

        f(&mut lower);

        debug_assert!(dependencies.is_none());
        self.state.scopes.pop();
        stmt_id
    }

    fn scope_expr<'child>(
        &'child mut self,
        target: impl IntoExprTarget<'child>,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx.scope(target),
            scope_id: self.scope_id,
            cx: self.cx,
            collect_dependencies: self.collect_dependencies,
        }
    }

    fn lower_insert<'child>(
        &'child mut self,
        target: &'child stmt::InsertTarget,
    ) -> LowerStatement<'child, 'b> {
        let columns = match target {
            stmt::InsertTarget::Scope(_) => {
                panic!("InsertTarget::Scope should already have been lowered by this point")
            }
            stmt::InsertTarget::Model(model_id) => &self.schema().mapping_for(model_id).columns,
            stmt::InsertTarget::Table(insert_table) => &insert_table.columns,
        };

        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx.scope(target),
            scope_id: self.scope_id,
            cx: LoweringContext::Insert(columns, None),
            collect_dependencies: self.collect_dependencies,
        }
    }

    fn lower_insert_with_row(&mut self, row: usize, f: impl FnOnce(&mut Self)) {
        let LoweringContext::Insert(_, maybe_row) = &mut self.cx else {
            todo!()
        };
        debug_assert!(maybe_row.is_none());
        *maybe_row = Some(row);
        f(self);

        let LoweringContext::Insert(_, maybe_row) = &mut self.cx else {
            todo!()
        };
        debug_assert_eq!(Some(row), *maybe_row);
        *maybe_row = None;
    }

    fn lower_insert_row<'child>(
        &'child mut self,
        row: &'child stmt::Expr,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::InsertRow(row),
            collect_dependencies: self.collect_dependencies,
        }
    }

    fn lower_returning(&mut self) -> LowerStatement<'_, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::Returning(None),
            collect_dependencies: self.collect_dependencies,
        }
    }

    fn lower_returning_for_row<'child>(
        &'child mut self,
        row_index: usize,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::Returning(Some(row_index)),
            collect_dependencies: self.collect_dependencies,
        }
    }

    fn cast_expr(&mut self, expr: &mut stmt::Expr, target_ty: &stmt::Type) {
        assert!(!target_ty.is_list(), "TODO");
        match expr {
            stmt::Expr::Cast(expr_cast) => {
                // TODO: verify that this is actually a correct cast.
                // Remove the cast - the inner expression is already the right type
                *expr = expr_cast.expr.take();
            }
            stmt::Expr::Value(value) => {
                // Cast the value to target_ty using existing cast method
                let casted = target_ty.cast(value.take()).expect("failed to cast value");
                *value = casted;
            }
            stmt::Expr::Project(_) => {
                todo!()
                // let base = expr.take();
                // *expr = stmt::Expr::cast(base, target_ty.clone());
            }
            stmt::Expr::Arg(_) => {
                // Create a cast expression for the arg
                let base = expr.take();
                *expr = stmt::Expr::cast(base, target_ty.clone());
            }
            _ => todo!("cast_expr: cannot cast {expr:#?} to {target_ty:?}"),
        }
    }
}

impl LoweringContext<'_> {
    fn is_insert(&self) -> bool {
        matches!(self, LoweringContext::Insert { .. })
    }

    fn is_returning(&self) -> bool {
        matches!(self, LoweringContext::Returning(_))
    }
}

/// Input implementation for assignment substitution.
///
/// Provides assignment values when substituting field references in `model_to_table`
/// expressions. Handles projections automatically for embedded fields.
struct AssignmentInput<'a> {
    assignment_projection: stmt::Projection,
    value: &'a stmt::Expr,
}

impl stmt::Input for AssignmentInput<'_> {
    fn resolve_ref(
        &mut self,
        expr_reference: &stmt::ExprReference,
        expr_projection: &stmt::Projection,
    ) -> Option<stmt::Expr> {
        let stmt::ExprReference::Field { nesting: 0, index } = expr_reference else {
            return None;
        };

        let assignment_steps = self.assignment_projection.as_slice();

        if *index != assignment_steps[0] {
            return None;
        }

        let remaining_steps = &assignment_steps[1..];

        if expr_projection.as_slice() == remaining_steps {
            Some(self.value.clone())
        } else {
            self.value.entry(expr_projection).map(|e| e.to_expr())
        }
    }
}

/// Builds the returning expression for a `Returning::Changed` update.
///
/// Iterates `mapping_fields` using `changed_bits` to determine what to include:
///
/// - Relation fields → null placeholder (populated during relation planning)
/// - Field whose `field_mask` is fully covered by `changed_bits` → emit
///   `project(ref_self_field, sub_projection)` (constantized later; project
///   omitted when `sub_projection` is identity)
/// - Field only partially covered → recurse to build a nested `SparseRecord`
///
/// `root_field_id` controls how the self-field reference is constructed:
/// - `None` at the top level: derived per-iteration as `{ model_id, i }`, since
///   each model field is its own root.
/// - `Some(id)` when recursing into an embedded type: fixed throughout, because
///   all sub-field expressions project from the same top-level ancestor field.
fn build_update_returning(
    model_id: app::ModelId,
    root_field_id: Option<app::FieldId>,
    mapping_fields: &[mapping::Field],
    changed_bits: &stmt::PathFieldSet,
) -> stmt::Expr {
    let mut exprs = vec![];
    let mut field_set = stmt::PathFieldSet::new();

    for (i, mf) in mapping_fields.iter().enumerate() {
        let intersection = changed_bits.clone() & mf.field_mask();

        if intersection.is_empty() {
            continue;
        }

        field_set.insert(i);

        if mf.is_relation() {
            // Relation field: null placeholder for the relation planner to fill
            // in via set_returning_field.
            exprs.push(stmt::Expr::null());
        } else {
            let root_field_id = root_field_id.unwrap_or(app::FieldId {
                model: model_id,
                index: i,
            });

            if intersection == mf.field_mask() {
                // Full coverage: all primitives in this field are being updated.
                // Emit a projected field reference; the lowering + constantize
                // pipeline will substitute the assignment value.
                let base = stmt::Expr::ref_self_field(root_field_id);
                let expr = if mf.sub_projection().is_identity() {
                    base
                } else {
                    stmt::Expr::project(base, mf.sub_projection().clone())
                };
                exprs.push(expr);
            } else {
                // Partial embedded update: only some sub-fields are changing.
                // Recurse to build a nested SparseRecord. The lowering pipeline
                // resolves each reference to the correct column, and the simplifier
                // folds project(record([...]), [i]) → column_ref.
                let emb_mapping = mf.as_struct().unwrap();
                exprs.push(build_update_returning(
                    model_id,
                    Some(root_field_id),
                    &emb_mapping.fields,
                    &intersection,
                ));
            }
        }
    }

    stmt::Expr::cast(
        stmt::ExprRecord::from_vec(exprs),
        stmt::Type::SparseRecord(field_set),
    )
}

/// True when an `IN` list is a candidate for the `= ANY($1)` rewrite:
///
/// - The lhs is scalar (not a `Record`). Composite-key `IN` would need a PG
///   row-array bind which the current driver doesn't support.
/// - The list is a constant collection of scalar values. Lists containing
///   references, sub-statements, or record values stay as expanded `IN`.
fn in_list_is_value_list(e: &stmt::ExprInList) -> bool {
    if matches!(*e.expr, stmt::Expr::Record(_)) {
        return false;
    }
    let scalar = |v: &stmt::Value| !matches!(v, stmt::Value::Record(_) | stmt::Value::List(_));
    match &*e.list {
        stmt::Expr::Value(stmt::Value::List(items)) => items.iter().all(scalar),
        stmt::Expr::List(list) => list
            .items
            .iter()
            .all(|i| matches!(i, stmt::Expr::Value(v) if scalar(v))),
        _ => false,
    }
}
