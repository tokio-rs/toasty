mod decompose;

mod info;

use info::{Arg, StatementInfoStore, StmtId};

mod materialize;
use materialize::{MaterializeGraph, MaterializeKind, NodeId};

use toasty_core::schema::db::ColumnId;
use toasty_core::stmt;

use crate::engine::eval;
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
struct PlannerNg<'a, 'b> {
    /// Stores decomposed statement info
    store: StatementInfoStore,

    /// Graph of materialization steps to execute the original statement being
    /// planned.
    graph: MaterializeGraph,

    /// TEMP: handle to the original planner (this will go away).
    old: &'a mut Planner<'b>,
}

impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, stmt: stmt::Statement) -> Result<plan::VarId> {
        PlannerNg {
            store: StatementInfoStore::new(),
            graph: MaterializeGraph::new(),
            old: self,
        }
        .plan_statement(stmt)
    }
}

impl PlannerNg<'_, '_> {
    fn plan_statement(&mut self, stmt: stmt::Statement) -> Result<plan::VarId> {
        self.decompose(stmt);

        // Build the execution plan...
        self.plan_materializations();

        // Build the execution plan
        for node_id in &self.graph.execution_order {
            let node = &self.graph[node_id];

            match &node.kind {
                MaterializeKind::Const(materialize_const) => {
                    // TODO: we probably want to optimize this using const folding

                    let var = self.old.var_table.register_var(node.ty().clone());
                    node.var.set(Some(var));

                    self.old.push_action(plan::SetVar {
                        var,
                        value: materialize_const.value.clone(),
                    });
                }
                MaterializeKind::ExecStatement(materialize_exec_statement) => {
                    debug_assert!(
                        {
                            match &materialize_exec_statement.stmt {
                                stmt::Statement::Query(query) => !query.single,
                                _ => true,
                            }
                        },
                        "as of now, no database can execute single queries"
                    );

                    let mut input_args = vec![];
                    let mut input_vars = vec![];

                    for input in &materialize_exec_statement.inputs {
                        let var = self.graph[input].var.get().unwrap();

                        input_args.push(self.old.var_table.ty(var).clone());
                        input_vars.push(var);
                    }

                    let ty = self
                        .old
                        .engine
                        .infer_ty(&materialize_exec_statement.stmt, &input_args);

                    let ty_fields = match &ty {
                        stmt::Type::List(ty_rows) => match &**ty_rows {
                            stmt::Type::Record(ty_fields) => ty_fields.clone(),
                            _ => todo!("ty={ty:#?}"),
                        },
                        _ => todo!("ty={ty:#?}"),
                    };
                    let var = self.old.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.old.push_action(plan::ExecStatement2 {
                        input: input_vars,
                        output: Some(plan::ExecStatementOutput { ty: ty_fields, var }),
                        stmt: materialize_exec_statement.stmt.clone(),
                    });
                }
                MaterializeKind::Filter(materialize_filter) => {
                    let input = self.graph.var_id(materialize_filter.input);
                    let ty = node.ty().clone();

                    let var = self.old.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.old.push_action(plan::Filter {
                        input,
                        output: var,
                        filter: materialize_filter.filter.clone(),
                    });
                }
                MaterializeKind::FindPkByIndex(materialize_find_pk_by_index) => {
                    let input = materialize_find_pk_by_index
                        .inputs
                        .iter()
                        .map(|node_id| self.graph.var_id(*node_id))
                        .collect();

                    let output = self.old.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.old.push_action(plan::FindPkByIndex2 {
                        input,
                        output,
                        table: materialize_find_pk_by_index.table,
                        index: materialize_find_pk_by_index.index,
                        filter: materialize_find_pk_by_index.filter.clone(),
                    });
                }
                MaterializeKind::GetByKey(materialize_get_by_key) => {
                    let input = self.graph.var_id(materialize_get_by_key.input);

                    let output = self.old.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = materialize_get_by_key
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column {
                                nesting,
                                table,
                                column,
                            } = expr_reference
                            else {
                                todo!()
                            };
                            debug_assert_eq!(*nesting, 0);
                            debug_assert_eq!(*table, 0);

                            ColumnId {
                                table: materialize_get_by_key.table,
                                index: *column,
                            }
                        })
                        .collect();

                    self.old.push_action(plan::GetByKey2 {
                        input,
                        output,
                        table: materialize_get_by_key.table,
                        columns,
                    });
                }
                MaterializeKind::NestedMerge(materialize_nested_merge) => {
                    let mut input_vars = vec![];

                    for input in &materialize_nested_merge.inputs {
                        let var = self.graph[input].var.get().unwrap();
                        input_vars.push(var);
                    }

                    let output = self.old.var_table.register_var(stmt::Type::list(
                        materialize_nested_merge.root.projection.ret.clone(),
                    ));
                    node.var.set(Some(output));

                    self.old.push_action(plan::NestedMerge {
                        inputs: input_vars,
                        output,
                        root: materialize_nested_merge.root.clone(),
                    });
                }
                MaterializeKind::Project(materialize_project) => {
                    let input_var = self.graph[materialize_project.input].var.get().unwrap();
                    let stmt::Type::List(input_ty) = self.old.var_table.ty(input_var).clone()
                    else {
                        todo!()
                    };

                    let input_args = vec![*input_ty];

                    let projection =
                        eval::Func::from_stmt(materialize_project.projection.clone(), input_args);
                    let var = self
                        .old
                        .var_table
                        .register_var(stmt::Type::list(projection.ret.clone()));
                    node.var.set(Some(var));

                    self.old.push_action(plan::Project {
                        input: input_var,
                        output: var,
                        projection,
                    });
                }
                MaterializeKind::QueryPk(materialize_query_pk) => {
                    let output = self.old.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = materialize_query_pk
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column {
                                nesting,
                                table,
                                column,
                            } = expr_reference
                            else {
                                todo!()
                            };
                            debug_assert_eq!(*nesting, 0);
                            debug_assert_eq!(*table, 0);

                            ColumnId {
                                table: materialize_query_pk.table,
                                index: *column,
                            }
                        })
                        .collect();

                    self.old.push_action(plan::QueryPk2 {
                        output,
                        table: materialize_query_pk.table,
                        columns,
                        pk_filter: materialize_query_pk.pk_filter.clone(),
                        row_filter: materialize_query_pk.row_filter.clone(),
                    });
                }
            }
        }

        let mid = self.store.root().output.get().unwrap();
        let output = self.graph[mid].var.get().unwrap();
        Ok(output)
    }
}
