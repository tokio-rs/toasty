mod info;
use info::{Arg, StatementInfoStore, StmtId};

mod materialize;
use materialize::{MaterializeGraph, MaterializeKind, NodeId};

mod lower;

mod var;
use var::VarTable;

use crate::{
    engine::{plan, Engine, Plan},
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
    store: StatementInfoStore,

    /// Graph of materialization steps to execute the original statement being
    /// planned.
    graph: MaterializeGraph,

    /// Table of record stream slots. Used to figure out where to store outputs
    /// of actions.
    var_table: VarTable,

    /// Actions that will end up in the pipeline.
    actions: Vec<plan::Action>,

    /// Variable to return as the result of the pipeline execution
    returning: Option<plan::VarId>,
}

impl Engine {
    pub(crate) fn plan(&self, stmt: stmt::Statement) -> Result<Plan> {
        let mut planner = Planner {
            engine: self,
            store: StatementInfoStore::new(),
            graph: MaterializeGraph::new(),
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

    fn plan_v2_stmt(&mut self, stmt: stmt::Statement) -> Result<Option<plan::VarId>> {
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

            match &node.kind {
                MaterializeKind::Const(materialize_const) => {
                    let var = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(var));

                    self.actions.push(
                        plan::SetVar {
                            output: plan::Output { var, num_uses },
                            rows: materialize_const.value.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::DeleteByKey(m) => {
                    let input = self.graph.var_id(m.input);
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        plan::DeleteByKey {
                            input,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            table: m.table,
                            filter: m.filter.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::ExecStatement(m) => {
                    debug_assert!(
                        {
                            match &m.stmt {
                                stmt::Statement::Query(query) => !query.single,
                                _ => true,
                            }
                        },
                        "as of now, no database can execute single queries"
                    );

                    let ty = node.ty();
                    let input_vars = m
                        .inputs
                        .iter()
                        .map(|input| self.graph[input].var.get().unwrap())
                        .collect();

                    let var = self.var_table.register_var(ty.clone());
                    node.var.set(Some(var));

                    let output_ty = match ty {
                        stmt::Type::List(ty_rows) => {
                            let ty_fields = match &**ty_rows {
                                stmt::Type::Record(ty_fields) => ty_fields.clone(),
                                _ => todo!("ty={ty:#?}; node={node:#?}"),
                            };

                            Some(ty_fields)
                        }
                        stmt::Type::Unit => None,
                        _ => todo!("ty={ty:#?}"),
                    };

                    self.actions.push(
                        plan::ExecStatement {
                            input: input_vars,
                            output: plan::ExecStatementOutput {
                                ty: output_ty,
                                output: plan::Output { var, num_uses },
                            },
                            stmt: m.stmt.clone(),
                            conditional_update_with_no_returning: m
                                .conditional_update_with_no_returning,
                        }
                        .into(),
                    );
                }
                MaterializeKind::Filter(materialize_filter) => {
                    let input = self.graph.var_id(materialize_filter.input);
                    let ty = node.ty().clone();

                    let var = self.var_table.register_var(ty);
                    node.var.set(Some(var));

                    self.actions.push(
                        plan::Filter {
                            input,
                            output: plan::Output { var, num_uses },
                            filter: materialize_filter.filter.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::FindPkByIndex(materialize_find_pk_by_index) => {
                    let input = materialize_find_pk_by_index
                        .inputs
                        .iter()
                        .map(|node_id| self.graph.var_id(*node_id))
                        .collect();

                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        plan::FindPkByIndex {
                            input,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            table: materialize_find_pk_by_index.table,
                            index: materialize_find_pk_by_index.index,
                            filter: materialize_find_pk_by_index.filter.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::GetByKey(materialize_get_by_key) => {
                    let input = self.graph.var_id(materialize_get_by_key.input);

                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = materialize_get_by_key
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column(expr_column) = expr_reference else {
                                todo!()
                            };
                            debug_assert_eq!(expr_column.nesting, 0);
                            debug_assert_eq!(expr_column.table, 0);

                            ColumnId {
                                table: materialize_get_by_key.table,
                                index: expr_column.column,
                            }
                        })
                        .collect();

                    self.actions.push(
                        plan::GetByKey {
                            input,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            table: materialize_get_by_key.table,
                            columns,
                        }
                        .into(),
                    );
                }
                MaterializeKind::NestedMerge(materialize_nested_merge) => {
                    let mut input_vars = vec![];

                    for input in &materialize_nested_merge.inputs {
                        let var = self.graph[input].var.get().unwrap();
                        input_vars.push(var);
                    }

                    let output = self.var_table.register_var(stmt::Type::list(
                        materialize_nested_merge.root.projection.ret.clone(),
                    ));
                    node.var.set(Some(output));

                    self.actions.push(
                        plan::NestedMerge {
                            inputs: input_vars,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            root: materialize_nested_merge.root.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::Project(materialize_project) => {
                    let input_var = self.graph[materialize_project.input].var.get().unwrap();

                    let var = self
                        .var_table
                        .register_var(stmt::Type::list(materialize_project.projection.ret.clone()));
                    node.var.set(Some(var));

                    self.actions.push(
                        plan::Project {
                            input: input_var,
                            output: plan::Output { var, num_uses },
                            projection: materialize_project.projection.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::ReadModifyWrite(m) => {
                    let input = m
                        .inputs
                        .iter()
                        .map(|input| self.graph[input].var.get().unwrap())
                        .collect();

                    // A hack since rmw doesn't support output yet
                    let var = self
                        .var_table
                        .register_var(stmt::Type::list(stmt::Type::Unit));

                    self.actions.push(
                        plan::ReadModifyWrite {
                            input,
                            output: Some(plan::Output { var, num_uses }),
                            read: m.read.clone(),
                            write: m.write.clone(),
                        }
                        .into(),
                    )
                }
                MaterializeKind::QueryPk(m) => {
                    let input = m.input.map(|node_id| self.graph.var_id(node_id));
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    let columns = m
                        .columns
                        .iter()
                        .map(|expr_reference| {
                            let stmt::ExprReference::Column(expr_column) = expr_reference else {
                                todo!()
                            };
                            debug_assert_eq!(expr_column.nesting, 0);
                            debug_assert_eq!(expr_column.table, 0);

                            ColumnId {
                                table: m.table,
                                index: expr_column.column,
                            }
                        })
                        .collect();

                    self.actions.push(
                        plan::QueryPk {
                            input,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            table: m.table,
                            columns,
                            pk_filter: m.pk_filter.clone(),
                            row_filter: m.row_filter.clone(),
                        }
                        .into(),
                    );
                }
                MaterializeKind::UpdateByKey(m) => {
                    let input = self.graph.var_id(m.input);
                    let output = self.var_table.register_var(node.ty().clone());
                    node.var.set(Some(output));

                    self.actions.push(
                        plan::UpdateByKey {
                            input,
                            output: plan::Output {
                                var: output,
                                num_uses,
                            },
                            table: m.table,
                            assignments: m.assignments.clone(),
                            filter: m.filter.clone(),
                            condition: m.condition.clone(),
                            returning: !m.ty.is_unit(),
                        }
                        .into(),
                    );
                }
            }
        }

        let mid = self.store.root().output.get().unwrap();
        Ok(self.graph[mid].var.get())
    }

    fn build(self) -> Result<Plan> {
        Ok(Plan {
            vars: exec::VarStore::new(self.var_table.into_vec()),
            actions: self.actions,
            returning: self.returning,
        })
    }
}
