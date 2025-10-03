mod materialization;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::usize;

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut, VisitMut};
use toasty_core::Schema;

use crate::engine::eval;
use crate::engine::planner::partition::materialization::MaterializationKind;
use crate::engine::{plan, planner::Planner};
use crate::Result;

/// Strategy for handling eager-loading of structurally-nested sub-statements
///
/// 1) Take a statement with nested sub-statements
/// 2) Break it down into an optimal sequence of materializations.
///     - Materializations load all the necessary records without any of the
///       final structure.
/// 3) Perform a nested-merge. This is a multi-step process.
///     - At each level, the "root materialization" should have all the
///       necessary fields for the merge qualification.
///     - Index the nested records, if necessary. For example, if the merge
///       qualification is an equality, create a hash map on the relevant
///       fields.
///     - For each root record, build the sub-statement result for the root
///       record by filtering the materialized nested records using the index /
///       merge qualification.
///     - Store the result of that record's sub-statement result as the input
///       referenced in the root statement's returning clause.
///         - Remember, earlier in the planning process we replaced the
///           statement in the returning clause with an ExprArg.
///     - Now we can use the projection extracted from the returning clause,
///       apply the materialized root record and the materialized nested records
///       for that level to get the final result.
/// 4) This nested-merge is represented as a new action type. The planner needs
///    to be able to describe the NestedMerge action.
///     - Materializations are stored in variables, they are referenced by the
///       NestedMerge action
///     - The NestedMerge needs the qualifications to perform the filter of the
///       materialization for the nested statement based on a single record from
///       the root statement.
///         - How should this merge qualification be represented? An Expr? If
///           so, how is the hash index used?
///     - The NestedMerge needs to know which argument to put the filtered
///       records in
///     - The NestedMerge needs to do the final projection.
/// 5) Deep nested merges are done recursively, from inside out. The planner can
///    create one NestedMerge action for each level, storing the result in a
///    variable. The next level pulls the input from that variable.
impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, mut stmt: stmt::Statement) -> Result<plan::VarId> {
        let mut walker_state = WalkerState {
            stmts: vec![StatementState::new()],
            scopes: vec![ScopeState { stmt_id: StmtId(0) }],
        };

        // Map the statement
        Walker {
            state: &mut walker_state,
            scope: 0,
            returning: false,
        }
        .visit_stmt_mut(&mut stmt);

        walker_state.stmts[0].stmt = Some(Box::new(stmt));

        // Build the execution plan...
        let materialization_graph = self.plan_materializations(&walker_state.stmts, StmtId(0));

        // Build the execution plan
        for node_id in &materialization_graph.execution_order {
            let node = &materialization_graph.nodes[*node_id];

            match &node.kind {
                MaterializationKind::ExecStatement { inputs, stmt } => {
                    let mut input_args = vec![];
                    let mut input_vars = vec![];

                    for input in inputs {
                        let var = materialization_graph.nodes[*input].var.get().unwrap();

                        input_args.push(self.var_table.ty(var).clone());
                        input_vars.push(var);
                    }

                    let args: Vec<_> = inputs
                        .iter()
                        .map(|node_id| {
                            let var = materialization_graph.nodes[*node_id].var.get().unwrap();
                            self.var_table.ty(var).clone()
                        })
                        .collect();

                    let ty = self.infer_ty(stmt, &args);
                    let var = self.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.push_action(plan::ExecStatement2 {
                        input: input_vars,
                        output: Some(var),
                        stmt: stmt.clone(),
                        conditional_update_with_no_returning: false,
                    });
                }
                MaterializationKind::Project { inputs, projection } => {
                    let mut input_args = vec![];
                    let mut input_vars = vec![];

                    for input in inputs {
                        let var = materialization_graph.nodes[*input].var.get().unwrap();

                        input_args.push(self.var_table.ty(var).clone());
                        input_vars.push(var);
                    }

                    let args: Vec<_> = inputs
                        .iter()
                        .map(|node_id| {
                            let var = materialization_graph.nodes[*node_id].var.get().unwrap();
                            self.var_table.ty(var).clone()
                        })
                        .collect();

                    let projection = eval::Func::from_stmt(projection.clone(), args);
                    let var = self.var_table.register_var(projection.ret.clone());
                    node.var.set(Some(var));

                    self.push_action(plan::Project {
                        input: input_vars,
                        output: var,
                        projection,
                    });
                }
            }
        }

        let mid = walker_state.stmts[0].output.get().unwrap();
        let output = materialization_graph.nodes[mid].var.get().unwrap();
        Ok(output)
    }
}

#[derive(Debug)]
struct Materialization {
    /// Mapped statement
    stmts: Vec<StatementState>,

    materializations: Vec<MaterializeStatement>,
}

#[derive(Debug)]
struct WalkerState {
    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    stmts: Vec<StatementState>,

    /// Scope state
    scopes: Vec<ScopeState>,
}

/// Per-statement state
#[derive(Debug)]
struct StatementState {
    /// Populated later
    stmt: Option<Box<stmt::Statement>>,

    /// Statement arguments
    args: Vec<Arg>,

    /// Sub-statements are statements declared within the definition of the
    /// containing statement.
    subs: Vec<StmtId>,

    /// Back-refs are expressions within sub-statements that reference the
    /// current statemetn.
    back_refs: HashMap<StmtId, BackRef>,

    /// Index of the ExecStatement materialization node for this statement.
    exec_statement: Cell<Option<usize>>,

    /// Index of the node that computes the final result for the statement
    output: Cell<Option<usize>>,

    /// Materialization
    materialization: usize,

    project: Option<stmt::Expr>,

    /// Merge qualification extracted before transformation
    /// This is extracted from the WHERE clause before VALUES(arg(0)) transformation
    merge_qualification: Option<plan::MergeQualification>,
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

#[derive(Debug)]
struct MaterializeStatement {
    /// Final query
    stmt: stmt::Statement,

    /// Expressions to return
    output: Vec<MaterializeOutput>,

    returnings: IndexSet<stmt::ExprReference>,

    /// Materializations it depends on
    deps: HashSet<usize>,

    /// Materialization return type. Because the materialization comes from the
    /// database, it is always a vec of values.
    ret_ty: Option<Vec<stmt::Type>>,
}

#[derive(Debug)]
struct MaterializeOutput {
    expr: stmt::Expr,
    var: Option<plan::VarId>,
}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut WalkerState,
    scope: usize,
    returning: bool,
}

#[derive(Debug, Default)]
struct BackRef {
    /// The expression
    exprs: IndexSet<stmt::ExprReference>,

    /// The projection node representing the output needed for this back-ref
    output: Option<usize>,
}

#[derive(Debug)]
enum Arg {
    /// A sub-statement
    Sub {
        /// The statement ID providing the input
        stmt_id: StmtId,

        /// The index in the materialization node's inputs list. This is set
        /// when planning materialization.
        input: Cell<Option<usize>>,
    },

    /// A back-reference
    Ref {
        /// The statement providing the data for the reference
        stmt_id: StmtId,
        /// The index of the column within the set of columns selected
        index: usize,

        /// The index in the materialization node's inputs list. This is set
        /// when planning materialization.
        input: Cell<Option<usize>>,
    },
}

#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
struct StmtId(usize);

impl<'a> visit_mut::VisitMut for Walker<'a> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column {
                    nesting,
                    table,
                    column,
                } = expr_reference
                else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    let stmt_id = self.curr_stmt_id();
                    let target_id = self.state.scopes[self.scope - *nesting].stmt_id;

                    // The reference is recreated assuming it is evaluated from the target context.
                    let expr = stmt::ExprReference::Column {
                        nesting: 0,
                        table: *table,
                        column: *column,
                    }
                    .into();

                    let index = self.stmt(target_id).new_back_ref(stmt_id, expr);
                    let arg_id = self.curr_stmt().new_ref_arg(target_id, index);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *i = stmt::Expr::arg(arg_id);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.returning);
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                println!("expr_stmt={expr_stmt:#?}");

                // Create a `StatementState` to track the sub-statement
                let target_id = self.new_stmt();
                let mut scope = self.scope(target_id);
                visit_mut::visit_expr_stmt_mut(&mut scope, expr_stmt);
                self.state.scopes.pop();

                // Create a new input to receive the statement
                let arg_id = self.curr_stmt().new_sub_stmt_arg(target_id);

                // Replace the sub-statement expression with a placeholder tracking the input
                let expr = std::mem::replace(i, stmt::Expr::arg(arg_id));
                let stmt::Expr::Stmt(expr_stmt) = expr else {
                    panic!()
                };
                self.stmt(target_id).stmt = Some(expr_stmt.stmt);
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        assert!(!self.returning);
        self.returning = true;
        visit_mut::visit_returning_mut(self, i);
        self.returning = false;
    }
}

impl<'a> Walker<'a> {
    fn new_stmt(&mut self) -> StmtId {
        let stmt_id = StmtId(self.state.stmts.len());
        self.state.stmts.push(StatementState::new());
        stmt_id
    }

    fn scope<'child>(&'child mut self, stmt_id: StmtId) -> Walker<'child> {
        let scope = self.state.scopes.len();
        self.state.scopes.push(ScopeState { stmt_id });

        Walker {
            state: self.state,
            scope,
            returning: false,
        }
    }

    fn curr_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope].stmt_id
    }

    fn curr_stmt(&mut self) -> &mut StatementState {
        &mut self.state.stmts[self.state.scopes[self.scope].stmt_id.0]
    }

    fn stmt(&mut self, stmt_id: StmtId) -> &mut StatementState {
        &mut self.state.stmts[stmt_id.0]
    }
}

/// What do I want to do here?
///
/// * Walk through the statement graph and identify cycles.
///     * Walk through *inputs*
///     * We need to flag **edges** as visited as well.
/// * For each node
///     * Flag node as visited
///     * Visit edge, mark edge as visited.
///     * If target is visited, we have a cycle break it.
///         * Split statement
///             * Keep all visited edges on old statement
///             * Move current edge + all unvisited edges to new statement
///     * If unvisited, recurse.
///
/// * We actually want to treat stmt returning as some sort of hash set so we
///   can easily modify it to add / remove items to satisfy the edges...
impl Materialization {
    fn plan_materialization(&mut self, stmt_id: StmtId) {
        let mid = self.new_materialization(stmt_id);

        let stmt = &**self.stmts[stmt_id.0].stmt.as_ref().unwrap();
        let stmt::Statement::Query(query) = stmt else {
            panic!()
        };
        let stmt::ExprSet::Select(select) = &query.body else {
            panic!()
        };
        let stmt::Returning::Expr(returning) = &select.returning else {
            panic!();
        };

        let materialization = &mut self.materializations[mid];

        // TODO: be smarter about the materialized record we request here. There
        // are plenty fo opportunities to avoid unnecessary vec clones
        // throughout the engine.
        let mut materialized_expr = vec![];
        let mut refs = IndexSet::new();

        let mut project = returning.clone();

        visit_mut::for_each_expr_mut(&mut project, |expr| {
            match expr {
                stmt::Expr::Reference(expr_reference) => {
                    let (index, inserted) = refs.insert_full(expr_reference.clone());

                    if inserted {
                        materialized_expr.push(expr.clone());
                    }

                    // First argument is for the primary materialization. Other
                    // arguments are for sub-statement materializations.
                    *expr = stmt::Expr::arg_project(0, [index]);
                }
                stmt::Expr::Arg(_expr_arg) => {
                    // These are sub-statements that we have to pull from other locations.

                    // This is a placeholder, it will result in a panic if we
                    // forget to deal with it.
                    *expr = stmt::Expr::arg(1);
                }
                _ => {}
            }
        });

        if !self.stmts[stmt_id.0].back_refs.is_empty() {
            // Essentially, we only handle one level for now
            assert!(
                self.stmts[stmt_id.0]
                    .args
                    .iter()
                    .all(|a| matches!(a, Arg::Sub { .. })),
                "TODO"
            );

            for back_ref in self.stmts[stmt_id.0].back_refs.values_mut() {
                assert_eq!(back_ref.exprs.len(), 1, "TODO");

                back_ref.output = Some(materialization.output.len());

                // Compute the record needed for this back ref
                let mut exprs = vec![];

                for expr_reference in &back_ref.exprs {
                    exprs.push(expr_reference.clone().into());
                }

                materialization.output.push(MaterializeOutput {
                    expr: stmt::Expr::record_from_vec(exprs),
                    var: None,
                });
            }
        }

        materialization.output.push(MaterializeOutput {
            expr: stmt::Expr::record_from_vec(materialized_expr),
            var: None,
        });

        // Plan materialization for all sub-statements.
        let stmt_state = self.stmt_state(stmt_id);
        stmt_state.project = Some(project);
        let subs = stmt_state.subs.clone();

        for sub in subs {
            self.plan_materialization(sub);
        }
    }

    fn stmt_state(&mut self, stmt_id: StmtId) -> &mut StatementState {
        &mut self.stmts[stmt_id.0]
    }

    /// Try to extract a merge qualification from a WHERE clause
    ///
    /// This looks for simple equality patterns like: child.parent_id = parent.id
    /// Returns Some(MergeQualification) if it can extract an equality, None otherwise
    fn try_extract_merge_qualification(
        &self,
        filter: &Option<stmt::Expr>,
        arg_position: usize,
    ) -> Option<plan::MergeQualification> {
        let filter = filter.as_ref()?;

        // For now, we only handle simple binary equality: child_col = parent_col
        // Future: handle AND of multiple equalities, complex predicates, etc.
        let stmt::Expr::BinaryOp(binary_op) = filter else {
            return None;
        };

        if !binary_op.op.is_eq() {
            return None;
        }

        // Check if we have: nested_column = project(arg(parent), [parent_col])
        let (nested_col, parent_col_idx) = match (&*binary_op.lhs, &*binary_op.rhs) {
            (
                stmt::Expr::Reference(stmt::ExprReference::Column {
                    nesting: 0,
                    table: _,
                    column: nested_idx,
                }),
                stmt::Expr::Project(proj),
            ) if matches!(&*proj.base, stmt::Expr::Arg(arg) if arg.position == arg_position) => {
                // Extract the column index from the projection
                // The projection should be a single index
                if let Some(idx) = Self::extract_single_index(&proj.projection) {
                    (nested_idx, idx)
                } else {
                    return None;
                }
            }
            (
                stmt::Expr::Project(proj),
                stmt::Expr::Reference(stmt::ExprReference::Column {
                    nesting: 0,
                    table: _,
                    column: nested_idx,
                }),
            ) if matches!(&*proj.base, stmt::Expr::Arg(arg) if arg.position == arg_position) => {
                // Extract the column index from the projection
                if let Some(idx) = Self::extract_single_index(&proj.projection) {
                    (nested_idx, idx)
                } else {
                    return None;
                }
            }
            _ => return None,
        };

        // For now, we return a placeholder that will be filled in during execution planning
        // We can't create the full MergeQualification yet because we don't have VarIds assigned
        // This will be converted to a proper qualification in plan_v2_stmt_execution
        Some(plan::MergeQualification::Equality {
            root_columns: vec![(0, parent_col_idx)], // 0 levels up = immediate parent
            index_id: plan::VarId(usize::MAX),       // Placeholder, will be filled later
        })
    }

    /// Extract a single index from a projection if it's a simple single-field projection
    fn extract_single_index(projection: &stmt::Projection) -> Option<usize> {
        // For now, we only handle simple projections
        // This will need to be expanded to handle more complex cases
        // The projection might be something like [0] which means "get field 0"
        // We need to look at the Projection implementation to understand its structure
        // For now, return None and we'll implement this properly later
        None // TODO: implement projection parsing
    }

    fn new_materialization(&mut self, stmt_id: StmtId) -> usize {
        let materialize_id = self.materializations.len();

        // Extract merge qualification BEFORE taking mutable borrow
        // to avoid borrow checker issues
        let merge_qualification = {
            let stmt_state = &self.stmts[stmt_id.0];
            let stmt = stmt_state.stmt.as_deref().unwrap();
            let stmt::Statement::Query(query) = stmt else {
                panic!()
            };
            let stmt::ExprSet::Select(select) = &query.body else {
                panic!()
            };

            // Find the arg index if there is one
            let arg_idx = stmt_state
                .args
                .iter()
                .enumerate()
                .find_map(|(i, arg)| matches!(arg, Arg::Ref { .. }).then_some(i));

            if let Some(i) = arg_idx {
                self.try_extract_merge_qualification(&Some(select.filter.clone()), i)
            } else {
                None
            }
        };

        // Now take mutable borrow
        let stmt_state = &mut self.stmts[stmt_id.0];
        stmt_state.merge_qualification = merge_qualification;

        let mut stmt = stmt_state.stmt.as_deref().unwrap().clone();

        let stmt::Statement::Query(query) = &mut stmt else {
            panic!()
        };
        let stmt::ExprSet::Select(select) = &mut query.body else {
            panic!()
        };

        for (i, arg) in stmt_state.args.iter().enumerate() {
            let Arg::Ref {
                stmt_id: _parent_stmt_id,
                index: _index,
                ..
            } = arg
            else {
                continue;
            };

            assert_eq!(1, stmt_state.args.len(), "TODO: handle more complex cases");

            // We rewrite the filter to batch load all possible records that
            // will be needed to materialize the original statement.
            /*
            -- Step 1: Store filtered users
            CREATE TEMP TABLE temp_users AS
            SELECT * FROM users WHERE users.active = true;

            -- Step 2: Fetch all potentially matching todos
            SELECT todos.*
            FROM todos
            WHERE EXISTS (
              SELECT 1 FROM temp_users u
              WHERE todos.user_id = u.id
              AND todos.created_at > u.created_at
              AND todos.priority > 3
            );
                 */

            visit_mut::for_each_expr_mut(&mut select.filter, |expr| {
                match expr {
                    stmt::Expr::Reference(stmt::ExprReference::Column { nesting, .. }) => {
                        // We need to up the nesting to reflect that the filter is moved
                        // one level deeper.
                        *nesting += 1;
                    }
                    stmt::Expr::Arg(expr_arg) if expr_arg.position == i => {
                        // Rewrite reference the new `FROM`.
                        // `column: 0` is hardcdoed for now
                        *expr = stmt::ExprReference::Column {
                            nesting: 0,
                            table: 0,
                            column: 0,
                        }
                        .into();
                    }
                    _ => {}
                }
            });

            let sub_select =
                stmt::Select::new(stmt::Values::from(stmt::Expr::arg(0)), select.filter.take());

            select.filter = stmt::Expr::exists(stmt::Query::builder(sub_select).returning(1));
        }

        stmt_state.materialization = materialize_id;

        self.materializations.push(MaterializeStatement {
            stmt,
            output: vec![],
            returnings: IndexSet::new(),
            deps: HashSet::new(),
            ret_ty: None,
        });

        materialize_id
    }
}

impl MaterializeStatement {
    fn compute_query(&mut self, schema: &Schema) {
        for output in &mut self.output {
            visit_mut::for_each_expr_mut(&mut output.expr, |expr| {
                match expr {
                    stmt::Expr::Reference(e) => {
                        // Track the needed reference and replace the expression with an argument that will pull from the position.
                        let (pos, _) = self.returnings.insert_full(e.clone());
                        *expr = stmt::Expr::arg(pos);
                    }
                    // Subqueries should have been removed at this point
                    stmt::Expr::Stmt(_) | stmt::Expr::InSubquery(_) => todo!(),
                    _ => {}
                }
            });
        }

        let stmt::Statement::Query(query) = &mut self.stmt else {
            todo!()
        };
        let stmt::ExprSet::Select(select) = &mut query.body else {
            todo!()
        };
        select.returning =
            stmt::Returning::from_expr_iter(self.returnings.iter().map(stmt::Expr::from));

        match stmt::ExprContext::new(schema).infer_stmt_ty(&self.stmt, &[]) {
            stmt::Type::List(ty) => match *ty {
                stmt::Type::Record(fields) => self.ret_ty = Some(fields),
                _ => todo!(),
            },
            stmt::Type::Unit => {}
            _ => todo!(),
        }
    }
}

impl StatementState {
    fn new() -> StatementState {
        StatementState {
            stmt: None,
            args: vec![],
            subs: vec![],
            back_refs: HashMap::new(),
            materialization: usize::MAX,
            project: None,
            merge_qualification: None,
            exec_statement: Cell::new(None),
            output: Cell::new(None),
        }
    }

    fn new_back_ref(&mut self, target_id: StmtId, expr: stmt::ExprReference) -> usize {
        let back_ref = self.back_refs.entry(target_id).or_default();
        let (ret, _) = back_ref.exprs.insert_full(expr);
        ret
    }

    fn new_ref_arg(&mut self, stmt_id: StmtId, index: usize) -> usize {
        let arg_id = self.args.len();
        self.args.push(Arg::Ref {
            stmt_id,
            index,
            input: Cell::new(None),
        });
        arg_id
    }

    fn new_sub_stmt_arg(&mut self, stmt_id: StmtId) -> usize {
        self.subs.push(stmt_id);
        let arg_id = self.args.len();
        self.args.push(Arg::Sub {
            stmt_id,
            input: Cell::new(None),
        });
        arg_id
    }
}
