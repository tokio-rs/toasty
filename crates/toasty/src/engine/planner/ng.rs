mod decompose;
mod materialize;

use std::cell::{Cell, OnceCell};
use std::collections::HashMap;

use indexmap::IndexSet;
use toasty_core::stmt::{self, ExprReference};

use crate::engine::eval;
use crate::engine::planner::ng::materialize::MaterializationKind;
use crate::engine::{plan, planner::Planner};
use crate::Result;

/// Planner for eager-loading nested sub-statements
///
/// # Overview
///
/// This planner transforms queries with nested sub-statements (e.g., loading users
/// with their todos) into an efficient execution plan that avoids N+1 queries.
///
/// # High-Level Strategy
///
/// Given a query like:
/// ```ignore
/// User::filter_by_active(true)
///     .include(User::FIELDS.todos())
///     .all(&db)
/// ```
///
/// The planner produces an execution plan represented as a directed acyclic graph (DAG)
/// of `MaterializationNode`s, where each node is one of:
///
/// - **ExecStatement**: Executes a database query, storing raw records in a variable
/// - **NestedMerge**: Combines parent and child records using qualification predicates
/// - **Project**: Transforms records using projection expressions
///
/// # Planning Process
///
/// ## Phase 1: Statement Decomposition (Walker)
///
/// The Walker visits the statement AST and:
/// - Identifies sub-statements (nested queries in RETURNING clause)
/// - Identifies back-references (expressions referencing parent scopes)
/// - Replaces sub-statements and back-refs with `ExprArg` placeholders
/// - Builds `StatementState` for each statement and sub-statement
///
/// ## Phase 2: Materialization Planning
///
/// For each statement, the planner:
///
/// 1. **Extracts columns needed** - walks the RETURNING clause to identify all
///    referenced columns, plus any columns needed for back-refs
///
/// 2. **Rewrites the query for batch loading** - if the statement has back-refs,
///    rewrites the WHERE clause to load all records that might match any parent:
///    ```sql
///    -- Original: SELECT * FROM todos WHERE todos.user_id = ?
///    -- Rewritten: SELECT * FROM todos WHERE EXISTS (
///    --   SELECT 1 FROM <parent_results> WHERE todos.user_id = <parent_results>.id
///    -- )
///    ```
///
/// 3. **Creates ExecStatement node** - the database query that loads records
///
/// 4. **Creates Project nodes for back-refs** - extracts just the columns needed
///    by child statements (used as input to NestedMerge)
///
/// 5. **Recurses into sub-statements** - plans each nested sub-statement
///
/// 6. **Creates NestedMerge node (if needed)** - if the statement has sub-statements,
///    creates a NestedMerge to combine parent and child records
///
/// ## Phase 3: NestedMerge Planning
///
/// The NestedMerge structure is recursive and describes how to:
///
/// - **Filter child records** - the `qualification` predicate determines which
///   child records match each parent record. Currently uses `Predicate` (nested loop),
///   but could be extended with hash-based joins for equality predicates.
///
/// - **Project results** - after filtering, the `projection` transforms the records
///   into the shape requested by the parent. Projections can reference:
///   - Arg 0: the current record's columns
///   - Arg 1+: results of recursive NestedMerge for this record's children
///
/// - **Recurse into children** - each `NestedChild` contains its own `NestedLevel`,
///   allowing arbitrarily deep nesting
///
/// # Execution Order
///
/// The planner computes a topological execution order that ensures:
/// 1. All ExecStatement nodes run first (can execute in parallel)
/// 2. NestedMerge nodes run after their input materializations complete
/// 3. Final Project node runs last to produce the user-visible result
///
/// # Example
///
/// For the User/Todos query above, the execution plan might be:
///
/// ```text
/// ExecStatement(users)        ExecStatement(todos)
///        |                            |
///        v                            v
///   Project(user back-refs)     [todos records]
///        |                            |
///        +---------> NestedMerge <----+
///                         |
///                         v
///                    [final result]
/// ```
impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, stmt: stmt::Statement) -> Result<plan::VarId> {
        let stmts = decompose::decompose(stmt);

        // Build the execution plan...
        let materialization_graph = self.plan_materializations(&stmts, StmtId(0));

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

        let mid = stmts[0].output.get().unwrap();
        let output = materialization_graph.nodes[mid].var.get().unwrap();
        Ok(output)
    }
}

/// Per-statement state
#[derive(Debug)]
struct StatementInfo {
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

impl StatementInfo {
    fn new() -> StatementInfo {
        StatementInfo {
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
