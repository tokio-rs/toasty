use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut};

use crate::engine::{
    eval,
    exec::{MergeIndex, MergeQualification, NestedChild, NestedLevel},
    hir, mir,
    plan::HirPlanner,
    Engine, HirStatement,
};

#[derive(Debug)]
struct NestedMergePlanner<'a> {
    engine: &'a Engine,
    hir: &'a HirStatement,
    mir: &'a mut mir::Store,
    inputs: IndexSet<mir::NodeId>,
    /// Statements that must execute before the merge but whose output is not needed
    deps: IndexSet<mir::NodeId>,
    /// Flat list of hash indexes to build, populated as HashLookup qualifications are planned.
    hash_indexes: Vec<MergeIndex>,
    /// Flat list of sorted indexes to build, populated as SortLookup qualifications are planned.
    sort_indexes: Vec<MergeIndex>,
    /// Statement stack, used to infer expression types
    stack: Vec<hir::StmtId>,
}

impl HirPlanner<'_> {
    /// Builds a nested merge operation for queries with sub-statement arguments
    /// in the returning clause.
    ///
    /// When a query has `Arg::Sub { returning: true, .. }` arguments
    /// (sub-statements used in the returning clause), those represent nested
    /// data that needs to be merged with their parent rows. This method
    /// constructs a `NestedMerge` execution plan that:
    ///
    /// 1. Identifies all batch-loaded inputs needed (parent and child queries)
    /// 2. Builds a tree structure mirroring the nesting hierarchy
    /// 3. For each level, captures:
    ///    - The source data (reference to batch-loaded results)
    ///    - How to filter child rows for each parent (qualification predicates)
    ///    - How to project the combined parent+children into the final shape
    ///
    /// The resulting `NestedMerge` will execute by:
    /// - Loading all batch data upfront - fetches all input data for all levels before processing
    /// - Processing each root row:
    ///   - For each nested child relationship, filters batch-loaded child data and recursively
    ///     merges matching rows with their own children
    ///   - Collects results into a list, or a single value if `single` is `true`
    ///   - Projects the final row with the current row and all nested children
    /// - Returning all merged rows with their nested data
    ///
    /// # Example
    ///
    /// For a query like:
    /// ```sql
    /// SELECT user.*, (SELECT * FROM todos WHERE user_id = user.id) as todos
    /// FROM users
    /// ```
    ///
    /// This builds a two-level merge where:
    /// - Root level: user rows from batch load
    /// - Nested level: todo rows filtered by user_id match, projected into a list
    ///
    /// Returns `None` if the statement has no sub-statements with `returning: true`.
    pub(super) fn plan_nested_merge(&mut self, stmt_id: hir::StmtId) -> Option<mir::NodeId> {
        let stmt_state = &self.hir[stmt_id];

        // Return if there is no nested merge to do
        let need_nested_merge = stmt_state.args.iter().any(|arg| {
            matches!(
                arg,
                hir::Arg::Sub {
                    returning: true,
                    ..
                }
            )
        });
        if !need_nested_merge {
            return None;
        }

        if stmt_state.stmt.as_ref().unwrap().is_insert() {
            // todo!("stmt_state={stmt_state:#?}");
            return None;
        }

        let nested_merge_planner = NestedMergePlanner {
            engine: self.engine,
            hir: self.hir,
            mir: &mut self.mir,
            inputs: IndexSet::new(),
            deps: IndexSet::new(),
            hash_indexes: vec![],
            sort_indexes: vec![],
            stack: vec![],
        };

        let node_id = nested_merge_planner.plan_nested_merge(stmt_id);
        Some(node_id)
    }
}

impl NestedMergePlanner<'_> {
    fn plan_nested_merge(mut self, root: hir::StmtId) -> mir::NodeId {
        self.stack.push(root);
        let root = self.plan_nested_level(root, 0);
        self.stack.pop();

        self.mir.insert_with_deps(
            mir::NestedMerge {
                inputs: self.inputs,
                root,
                hash_indexes: self.hash_indexes,
                sort_indexes: self.sort_indexes,
            },
            self.deps,
        )
    }

    fn plan_nested_child(&mut self, stmt_id: hir::StmtId, nullable: bool, depth: usize) -> NestedChild {
        self.stack.push(stmt_id);

        let level = self.plan_nested_level(stmt_id, depth);
        let stmt_state = &self.hir[stmt_id];
        let selection = stmt_state.load_data_columns.get().unwrap();

        let ret = match stmt_state.stmt.as_deref().unwrap() {
            stmt::Statement::Query(query) => {
                let filter_expr = self.build_filter_for_nested_child(stmt_id, selection, depth);

                let filter_arg_tys = self.build_filter_arg_tys();
                let qualification = match try_eq_lookup(&filter_expr, &filter_arg_tys, depth) {
                    Some((child_projections, lookup_key)) if query.single => {
                        // has_one / belongs_to: unique key → HashIndex (O(1) lookup).
                        let index = self.hash_indexes.len();
                        self.hash_indexes.push(MergeIndex {
                            source: level.source,
                            child_projections,
                        });
                        MergeQualification::HashLookup { index, lookup_key }
                    }
                    Some((child_projections, lookup_key)) => {
                        // has_many: duplicate keys → SortedIndex (O(log M + k) lookup).
                        let index = self.sort_indexes.len();
                        self.sort_indexes.push(MergeIndex {
                            source: level.source,
                            child_projections,
                        });
                        MergeQualification::SortLookup { index, lookup_key }
                    }
                    // Filter does not reduce to a pure equality conjunction, so we
                    // cannot drive an index lookup. Fall back to a linear scan.
                    // See `try_eq_lookup` for discussion of how this could be
                    // improved to use an index with a residual post-filter.
                    None => {
                        MergeQualification::Scan(eval::Func::from_stmt(filter_expr, filter_arg_tys))
                    }
                };

                NestedChild {
                    level,
                    qualification,
                    single: query.single,
                    nullable,
                }
            }
            stmt::Statement::Insert(insert) => NestedChild {
                level,
                qualification: MergeQualification::All,
                single: insert.source.single,
                nullable,
            },
            stmt => todo!("stmt={stmt:#?}"),
        };

        self.stack.pop();

        ret
    }

    fn plan_nested_level(&mut self, stmt_id: hir::StmtId, depth: usize) -> NestedLevel {
        let stmt_state = &self.hir[stmt_id];
        let stmt = stmt_state.stmt.as_deref().unwrap();
        let returning = stmt.returning_unwrap();

        let source;
        let mut nested = vec![];

        // Map the returning clause to projection expression
        let projection = match returning {
            stmt::Returning::Expr(expr) => {
                let (s, _) = self
                    .inputs
                    .insert_full(stmt_state.load_data_statement.get().unwrap());

                source = s;
                self.build_projection_from_expr(stmt_id, expr, depth, &mut nested)
            }
            _ => {
                let node_id = stmt_state.output.get().unwrap();

                let (s, _) = self.inputs.insert_full(node_id);
                source = s;

                // Flatten list (bit of a hack)
                let ty = match self.mir[node_id].ty().clone() {
                    stmt::Type::List(ty) => *ty,
                    ty => ty,
                };

                eval::Func::from_stmt(stmt::Expr::arg(0), vec![ty])
            }
        };

        NestedLevel {
            source,
            projection,
            nested,
        }
    }

    fn build_filter_arg_tys(&self) -> Vec<stmt::Type> {
        self.stack
            .iter()
            .map(|stmt_id| self.build_exec_statement_ty_for(*stmt_id))
            .collect()
    }

    fn build_projection_arg_tys(&self, nested_children: &[NestedChild]) -> Vec<stmt::Type> {
        let curr = self.stack.last().unwrap();
        let mut projection_arg_tys = vec![self.build_exec_statement_ty_for(*curr)];

        for nested in nested_children {
            projection_arg_tys.push(if nested.single && !nested.nullable {
                nested.level.projection.ret.clone()
            } else {
                stmt::Type::list(nested.level.projection.ret.clone())
            });
        }

        projection_arg_tys
    }

    fn build_exec_statement_ty_for(&self, stmt_id: hir::StmtId) -> stmt::Type {
        let stmt_state = &self.hir[stmt_id];
        let stmt = stmt_state.stmt.as_deref().unwrap();

        let cx = stmt::ExprContext::new_with_target(&*self.engine.schema, stmt);

        let mut fields = vec![];

        for expr_reference in stmt_state.load_data_columns.get().unwrap() {
            fields.push(cx.infer_expr_reference_ty(expr_reference));
        }

        stmt::Type::Record(fields)
    }

    fn build_projection_from_expr(
        &mut self,
        stmt_id: hir::StmtId,
        expr: &stmt::Expr,
        depth: usize,
        nested: &mut Vec<NestedChild>,
    ) -> eval::Func {
        let stmt_state = &self.hir[stmt_id];
        let selection = stmt_state.load_data_columns.get().unwrap();
        let mut projection = expr.clone();

        visit_mut::for_each_expr_mut(&mut projection, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => match &stmt_state.args[expr_arg.position] {
                hir::Arg::Sub { stmt_id, nullable, .. } => {
                    let nullable = *nullable;
                    let child_stmt_state = &self.hir[stmt_id];
                    let child_stmt = child_stmt_state.stmt.as_deref().unwrap();
                    let child_returning = child_stmt.returning_unwrap();

                    // If the child statement has a constant returning clause,
                    // then the nested merge can inline the returning directly
                    // instead of having to get the values from the expression.
                    match child_returning {
                        stmt::Returning::Value(returning_expr) if returning_expr.is_const() => {
                            match child_stmt {
                                stmt::Statement::Query(query) => {
                                    if query.single {
                                        let stmt::Expr::Value(v) = returning_expr else {
                                            todo!()
                                        };
                                        assert!(!v.is_list());
                                    }
                                }
                                stmt::Statement::Insert(insert) => {
                                    if insert.source.single {
                                        let stmt::Expr::Value(v) = returning_expr else {
                                            todo!()
                                        };
                                        assert!(!v.is_list());
                                    }
                                }
                                _ => {}
                            }

                            // For consistency, make sure the child statement's execution happens before this one.
                            self.deps
                                .insert(child_stmt_state.load_data_statement.get().unwrap());
                            *expr = returning_expr.clone();
                        }
                        _ => {
                            let nested_child = self.plan_nested_child(*stmt_id, nullable, depth + 1);
                            nested.push(nested_child);

                            // Taking the
                            *expr = stmt::Expr::arg(nested.len());
                        }
                    }
                }
                hir::Arg::Ref { .. } => todo!(),
            },
            stmt::Expr::Reference(expr_reference) => {
                let expr_column = expr_reference.as_expr_column_unwrap();
                debug_assert_eq!(0, expr_column.nesting);
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(0, [index]);
            }
            _ => {}
        });

        let projection_arg_tys = self.build_projection_arg_tys(nested);
        eval::Func::from_stmt(projection, projection_arg_tys)
    }

    fn build_filter_for_nested_child(
        &self,
        stmt_id: hir::StmtId,
        selection: &IndexSet<stmt::ExprReference>,
        depth: usize,
    ) -> stmt::Expr {
        let stmt_state = &self.hir[stmt_id];
        let stmt::Statement::Query(query) = stmt_state.stmt.as_deref().unwrap() else {
            unreachable!()
        };
        let select = query.body.as_select_unwrap();

        // Extract the qualification. For now, we will just re-run the
        // entire where clause, but that can be improved later.
        let mut filter = select.filter.clone();

        visit_mut::for_each_expr_mut(&mut filter, |expr| match expr {
            stmt::Expr::Arg(expr_arg) => {
                let hir::Arg::Ref {
                    nesting,
                    stmt_id: target_id,
                    target_expr_ref,
                    ..
                } = &stmt_state.args[expr_arg.position]
                else {
                    todo!()
                };

                debug_assert!(*nesting > 0);

                // This is a bit of a roundabout way to get the data. We may
                // want to find a better way to track the info for more direct
                // access.
                let target_stmt = &self.hir[target_id];

                let target_exec_statement_index = target_stmt
                    .load_data_columns
                    .get()
                    .unwrap()
                    .get_index_of(target_expr_ref)
                    .unwrap();

                *expr = stmt::Expr::arg_project(depth - *nesting, [target_exec_statement_index]);
            }
            stmt::Expr::Reference(expr_reference) => {
                let index = selection.get_index_of(expr_reference).unwrap();
                *expr = stmt::Expr::arg_project(depth, [index]);
            }
            _ => {}
        });

        filter.into_expr()
    }
}

/// Try to extract index lookup key fields from a transformed filter expression.
///
/// Recognizes patterns of the form:
/// - Single equality: `arg_project(depth, [cf]) == arg_project(pos < depth, [pf])`
/// - Composite AND:   `eq1 AND eq2 AND ...` where each `eqi` has the above form
///
/// On success returns `(child_projections, lookup_key)` where:
/// - `child_projections[i]` is the projection into the child record for key field `i`
/// - `lookup_key` is an `eval::Func` that evaluates against the ancestor `RowStack`
///   and returns the lookup key (scalar for single-field, `Value::Record` for composite)
///
/// # Limitations
///
/// This function only succeeds when the *entire* filter is a pure equality conjunction.
/// Any more complex filter (e.g. `a = b AND c > d`, or an `OR`) causes it to return
/// `None` and the caller falls back to a full `Scan`, even if part of the filter could
/// drive an index lookup with the remainder applied as a post-filter.
///
/// A more complete approach — similar to `IndexMatch` in the index planner — would
/// extract whichever equality terms can key an index, build the lookup from those,
/// and re-evaluate the full original predicate against each candidate row returned by
/// the index. That would turn O(N×M) into O(log M + k) even for compound filters.
/// For now we keep this conservative: only use an index when the whole filter matches.
fn try_eq_lookup(
    expr: &stmt::Expr,
    arg_tys: &[stmt::Type],
    depth: usize,
) -> Option<(Vec<stmt::Projection>, eval::Func)> {
    // Collect equality terms: single BinaryOp(Eq) or AND of BinaryOp(Eq)s.
    let eq_terms: Vec<(&stmt::Expr, &stmt::Expr)> = match expr {
        stmt::Expr::BinaryOp(op) if op.op == stmt::BinaryOp::Eq => {
            vec![(&op.lhs, &op.rhs)]
        }
        stmt::Expr::And(and_expr) => {
            let mut terms = vec![];
            for operand in and_expr.operands.iter() {
                match operand {
                    stmt::Expr::BinaryOp(op) if op.op == stmt::BinaryOp::Eq => {
                        terms.push((&*op.lhs, &*op.rhs));
                    }
                    _ => return None,
                }
            }
            terms
        }
        _ => return None,
    };

    let mut child_projections = vec![];
    let mut lookup_key_exprs = vec![];

    for (lhs, rhs) in eq_terms {
        let (child_proj, parent_expr) = extract_child_parent_eq(lhs, rhs, depth)?;
        child_projections.push(child_proj);
        lookup_key_exprs.push(parent_expr);
    }

    if child_projections.is_empty() {
        return None;
    }

    // Build the parent key expression. For a single field, use the scalar
    // directly. For multiple fields, wrap in a record (evaluates to Value::Record).
    let lookup_key_expr = if lookup_key_exprs.len() == 1 {
        lookup_key_exprs.remove(0)
    } else {
        stmt::Expr::record_from_vec(lookup_key_exprs)
    };

    // Parent key args are the ancestor stack types only (not including the
    // current child row at position `depth`).
    let lookup_key_arg_tys = arg_tys[..depth].to_vec();
    let lookup_key = eval::Func::from_stmt(lookup_key_expr, lookup_key_arg_tys);

    Some((child_projections, lookup_key))
}

/// For an equality `lhs == rhs`, determine which side is the child (at `depth`)
/// and which is the parent (at some position < `depth`).
///
/// Both sides must be simple `arg_project(pos, projection)` expressions.
/// Returns `(child_projection, parent_expr)` or `None` if the pattern doesn't match.
fn extract_child_parent_eq(
    lhs: &stmt::Expr,
    rhs: &stmt::Expr,
    depth: usize,
) -> Option<(stmt::Projection, stmt::Expr)> {
    match (as_simple_arg_project(lhs), as_simple_arg_project(rhs)) {
        (Some((l_pos, l_proj)), Some((r_pos, _))) if l_pos == depth && r_pos < depth => {
            Some((l_proj.clone(), rhs.clone()))
        }
        (Some((l_pos, _)), Some((r_pos, r_proj))) if r_pos == depth && l_pos < depth => {
            Some((r_proj.clone(), lhs.clone()))
        }
        _ => None,
    }
}

/// Match `Project(Arg { position, nesting: 0 }, projection)` and return
/// `(position, &projection)`. Returns `None` for any other expression shape.
fn as_simple_arg_project(expr: &stmt::Expr) -> Option<(usize, &stmt::Projection)> {
    match expr {
        stmt::Expr::Project(proj) => match proj.base.as_ref() {
            stmt::Expr::Arg(arg) if arg.nesting == 0 => Some((arg.position, &proj.projection)),
            _ => None,
        },
        _ => None,
    }
}
