mod hir;

mod materialize;

mod mir;

mod lower;

mod var;
use var::VarTable;

use crate::{
    engine::{
        exec::{ExecPlan, VarId},
        Engine,
    },
    Result,
};
use toasty_core::{
    schema::db::ColumnId,
    stmt::{self},
    Schema,
};

use super::exec;

#[derive(Debug)]
struct Planner<'a> {
    /// Handle to the schema & driver capabilities.
    engine: &'a Engine,

    /// Stores decomposed statement info
    store: hir::Store,

    /// Graph of materialization steps to execute the original statement being
    /// planned.
    graph: mir::MaterializeGraph,

    /// Table of record stream slots. Used to figure out where to store outputs
    /// of actions.
    var_table: VarTable,

    /// Actions that will end up in the pipeline.
    actions: Vec<exec::Action>,

    /// Variable to return as the result of the pipeline execution
    returning: Option<exec::VarId>,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<ExecPlan> {
        let mut planner = Planner {
            engine: self,
            store: hir::Store::new(),
            graph: mir::MaterializeGraph::new(),
            var_table: VarTable::default(),
            actions: vec![],
            returning: None,
        };

        planner.plan_stmt_root(stmt)?;
        planner.build()
    }
}

impl<'a> Planner<'a> {
    pub(crate) fn schema(&self) -> &'a Schema {
        &self.engine.schema
    }

    /// Entry point to plan the root statement.
    fn plan_stmt_root(&mut self, stmt: stmt::Statement) -> Result<()> {
        if let stmt::Statement::Insert(stmt) = &stmt {
            assert!(matches!(
                stmt.returning,
                Some(stmt::Returning::Model { .. })
            ));
        }

        let output = self.plan_v2_stmt(stmt)?;

        if let Some(output) = output {
            self.returning = Some(output);
        }

        Ok(())
    }

    fn plan_v2_stmt(&mut self, stmt: stmt::Statement) -> Result<Option<VarId>> {
        self.lower_stmt(stmt)?;

        // Build the execution plan...
        self.plan_materializations();

        let mid = self.store.root().output.get().unwrap();
        let node = &self.graph[mid];
        node.num_uses.set(node.num_uses.get() + 1);

        // Build the execution plan
        for node_id in &self.graph.execution_order {
            let node = &self.graph[node_id];
            let num_uses = node.num_uses.get();

            match &node.op {
                mir::Operation::Const(op) => {
                    let action = op.to_exec(node, &mut self.var_table);
                    self.actions.push(action.into());
                }
                mir::Operation::DeleteByKey(op) => {
                    let input = self.graph.var_id(op.input);
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        exec::DeleteByKey {
                            input,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            table: op.table,
                            filter: op.filter.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::ExecStatement(op) => {
                    let action = op.to_exec(&self.graph, node, &mut self.var_table);
                    self.actions.push(action.into());
                }
                mir::Operation::Filter(op) => {
                    let input = self.graph.var_id(op.input);
                    let ty = node.ty().clone();

                    let var = self.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.actions.push(
                        exec::Filter {
                            input,
                            output: exec::Output { var, num_uses },
                            filter: op.filter.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::FindPkByIndex(op) => {
                    let input = op
                        .inputs
                        .iter()
                        .map(|node_id| self.graph.var_id(*node_id))
                        .collect();

                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        exec::FindPkByIndex {
                            input,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            table: op.table,
                            index: op.index,
                            filter: op.filter.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::GetByKey(op) => {
                    let input = self.graph.var_id(op.input);

                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = op
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column(expr_column) = expr_reference else {
                                todo!()
                            };
                            debug_assert_eq!(expr_column.nesting, 0);
                            debug_assert_eq!(expr_column.table, 0);

                            ColumnId {
                                table: op.table,
                                index: expr_column.column,
                            }
                        })
                        .collect();

                    self.actions.push(
                        exec::GetByKey {
                            input,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            table: op.table,
                            columns,
                        }
                        .into(),
                    );
                }
                mir::Operation::NestedMerge(op) => {
                    let mut input_vars = vec![];

                    for input in &op.inputs {
                        let var = self.graph[input].var.get().unwrap();
                        input_vars.push(var);
                    }

                    let output = self
                        .var_table
                        .register_var(stmt::Type::list(op.root.projection.ret.clone()));
                    node.var.set(Some(output));

                    self.actions.push(
                        exec::NestedMerge {
                            inputs: input_vars,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            root: op.root.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::Project(op) => {
                    let input_var = self.graph[op.input].var.get().unwrap();

                    let var = self
                        .var_table
                        .register_var(stmt::Type::list(op.projection.ret.clone()));
                    node.var.set(Some(var));

                    self.actions.push(
                        exec::Project {
                            input: input_var,
                            output: exec::Output { var, num_uses },
                            projection: op.projection.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::ReadModifyWrite(op) => {
                    let input = op
                        .inputs
                        .iter()
                        .map(|input| self.graph[input].var.get().unwrap())
                        .collect();

                    // A hack since rmw doesn't support output yet
                    let var = self
                        .var_table
                        .register_var(stmt::Type::list(stmt::Type::Unit));

                    self.actions.push(
                        exec::ReadModifyWrite {
                            input,
                            output: Some(exec::Output { var, num_uses }),
                            read: op.read.clone(),
                            write: op.write.clone(),
                        }
                        .into(),
                    )
                }
                mir::Operation::QueryPk(op) => {
                    let input = op.input.map(|node_id| self.graph.var_id(node_id));
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = op
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column(expr_column) = expr_reference else {
                                todo!()
                            };
                            debug_assert_eq!(expr_column.nesting, 0);
                            debug_assert_eq!(expr_column.table, 0);

                            ColumnId {
                                table: op.table,
                                index: expr_column.column,
                            }
                        })
                        .collect();

                    self.actions.push(
                        exec::QueryPk {
                            input,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            table: op.table,
                            columns,
                            pk_filter: op.pk_filter.clone(),
                            row_filter: op.row_filter.clone(),
                        }
                        .into(),
                    );
                }
                mir::Operation::UpdateByKey(op) => {
                    let input = self.graph.var_id(op.input);
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        exec::UpdateByKey {
                            input,
                            output: exec::Output {
                                var: output,
                                num_uses,
                            },
                            table: op.table,
                            assignments: op.assignments.clone(),
                            filter: op.filter.clone(),
                            condition: op.condition.clone(),
                            returning: !op.ty.is_unit(),
                        }
                        .into(),
                    );
                }
            }
        }

        let mid = self.store.root().output.get().unwrap();
        Ok(self.graph[mid].var.get())
    }

    fn build(self) -> Result<ExecPlan> {
        Ok(ExecPlan {
            vars: exec::VarStore::new(self.var_table.into_vec()),
            actions: self.actions,
            returning: self.returning,
        })
    }
}
