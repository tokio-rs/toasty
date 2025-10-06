mod materialization;

use std::cell::{Cell, OnceCell};
use std::collections::HashMap;
use std::usize;

use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut, ExprReference, VisitMut};

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
                MaterializationKind::ExecStatement { inputs, stmt, .. } => {
                    debug_assert!(
                        {
                            match &stmt {
                                stmt::Statement::Query(query) => !query.single,
                                _ => true,
                            }
                        },
                        "as of now, no database can execute single queries"
                    );

                    let mut input_args = vec![];
                    let mut input_vars = vec![];

                    for input in inputs {
                        let var = materialization_graph.nodes[*input].var.get().unwrap();

                        input_args.push(self.var_table.ty(var).clone());
                        input_vars.push(var);
                    }

                    let ty = self.infer_ty(stmt, &input_args);

                    let ty_fields = match &ty {
                        stmt::Type::List(ty_rows) => match &**ty_rows {
                            stmt::Type::Record(ty_fields) => ty_fields.clone(),
                            _ => todo!("ty={ty:#?}"),
                        },
                        _ => todo!("ty={ty:#?}"),
                    };
                    let var = self.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.push_action(plan::ExecStatement2 {
                        input: input_vars,
                        output: Some(plan::ExecStatementOutput { ty: ty_fields, var }),
                        stmt: stmt.clone(),
                    });
                }
                MaterializationKind::NestedMerge { inputs, root, .. } => {
                    let mut input_vars = vec![];

                    for input in inputs {
                        let var = materialization_graph.nodes[*input].var.get().unwrap();
                        input_vars.push(var);
                    }

                    let output = self
                        .var_table
                        .register_var(stmt::Type::list(root.projection.ret.clone()));
                    node.var.set(Some(output));

                    self.push_action(plan::NestedMerge {
                        inputs: input_vars,
                        output,
                        root: root.clone(),
                    });
                }
                MaterializationKind::Project { input, projection } => {
                    let input_var = materialization_graph.nodes[*input].var.get().unwrap();
                    let stmt::Type::List(input_ty) = self.var_table.ty(input_var).clone() else {
                        todo!()
                    };

                    let input_args = vec![*input_ty];

                    let projection = eval::Func::from_stmt(projection.clone(), input_args);
                    let var = self
                        .var_table
                        .register_var(stmt::Type::list(projection.ret.clone()));
                    node.var.set(Some(var));

                    self.push_action(plan::Project {
                        input: input_var,
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

    /// Columns selected by exec_statement
    exec_statement_selection: OnceCell<IndexSet<ExprReference>>,

    /// Index of the node that computes the final result for the statement
    output: Cell<Option<usize>>,
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
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

    /// Projection materialization node ID
    node_id: Cell<Option<usize>>,
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

        /// The nesting level
        nesting: usize,

        /// The index of the column within the set of columns included during
        /// the batch-load query.
        batch_load_index: usize,

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

                    let batch_load_index = self.stmt(target_id).new_back_ref(stmt_id, expr);
                    let arg_id =
                        self.curr_stmt()
                            .new_ref_arg(target_id, *nesting, batch_load_index);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *i = stmt::Expr::arg(arg_id);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.returning);
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

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

impl StatementState {
    fn new() -> StatementState {
        StatementState {
            stmt: None,
            args: vec![],
            subs: vec![],
            back_refs: HashMap::new(),
            exec_statement: Cell::new(None),
            exec_statement_selection: OnceCell::new(),
            output: Cell::new(None),
        }
    }

    fn new_back_ref(&mut self, target_id: StmtId, expr: stmt::ExprReference) -> usize {
        let back_ref = self.back_refs.entry(target_id).or_default();
        let (ret, _) = back_ref.exprs.insert_full(expr);
        ret
    }

    fn new_ref_arg(&mut self, stmt_id: StmtId, nesting: usize, batch_load_index: usize) -> usize {
        let arg_id = self.args.len();
        self.args.push(Arg::Ref {
            stmt_id,
            nesting,
            batch_load_index,
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
