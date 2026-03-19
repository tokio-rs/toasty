use crate::engine::{
    exec::{self, BlockBuilder, ExecPlan, Terminator, VarSource, VarStore},
    mir::Operation,
    plan::ExecPlanner,
};

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> ExecPlan {
        let mut bb = BlockBuilder::new();
        let mut current_block = bb.new_block();

        for &node_id in self.logical_plan.execution_order() {
            let node = &self.logical_plan[node_id];

            match &node.op {
                Operation::Branch(branch) => {
                    // Register a var for the Branch node's output.
                    // Both branches will write to this var.
                    let output_var = self.var_decls.register_var(branch.ty.clone());
                    node.var.set(Some(output_var));
                    let output_num_uses = node.num_uses.get();

                    // The cond node has already been processed; get its var.
                    let cond_var = self.logical_plan[branch.cond].var.get().unwrap();

                    // Create then, else, and merge blocks.
                    let then_block = bb.new_block();
                    let else_block = bb.new_block();
                    let merge_block = bb.new_block();

                    // End the current block with the conditional branch.
                    bb.set_terminator(
                        current_block,
                        Terminator::If {
                            cond: cond_var,
                            then_block,
                            else_block,
                        },
                    );

                    // Process then_body nodes into the then block.
                    for &body_node_id in &branch.then_body {
                        let body_node = &self.logical_plan[body_node_id];
                        let action = body_node.to_exec(self.logical_plan, &mut self.var_decls);
                        bb.push_action(then_block, action);
                    }

                    // Copy then_output result to the branch output var.
                    let then_result_var = self.logical_plan[branch.then_output].var.get().unwrap();
                    bb.push_action(
                        then_block,
                        exec::SetVar {
                            source: VarSource::Var(then_result_var),
                            output: exec::Output {
                                var: output_var,
                                num_uses: output_num_uses,
                            },
                        }
                        .into(),
                    );
                    bb.set_terminator(then_block, Terminator::Goto(merge_block));

                    // Process else_body nodes into the else block.
                    for &body_node_id in &branch.else_body {
                        let body_node = &self.logical_plan[body_node_id];
                        let action = body_node.to_exec(self.logical_plan, &mut self.var_decls);
                        bb.push_action(else_block, action);
                    }

                    if let Some(else_out) = branch.else_output {
                        // Copy else_output result to the branch output var.
                        let else_result_var = self.logical_plan[else_out].var.get().unwrap();
                        bb.push_action(
                            else_block,
                            exec::SetVar {
                                source: VarSource::Var(else_result_var),
                                output: exec::Output {
                                    var: output_var,
                                    num_uses: output_num_uses,
                                },
                            }
                            .into(),
                        );
                    } else if branch.ty.is_unit() {
                        // Unit-typed branches use Count(0) for the else case.
                        bb.push_action(
                            else_block,
                            exec::SetVar {
                                source: VarSource::Count(0),
                                output: exec::Output {
                                    var: output_var,
                                    num_uses: output_num_uses,
                                },
                            }
                            .into(),
                        );
                    } else {
                        // Else branch produces a constant value.
                        bb.push_action(
                            else_block,
                            exec::SetVar {
                                source: VarSource::Value(branch.else_value.clone()),
                                output: exec::Output {
                                    var: output_var,
                                    num_uses: output_num_uses,
                                },
                            }
                            .into(),
                        );
                    }
                    bb.set_terminator(else_block, Terminator::Goto(merge_block));

                    // Continue placing subsequent actions into the merge block.
                    current_block = merge_block;
                }
                _ => {
                    let action = node.to_exec(self.logical_plan, &mut self.var_decls);
                    bb.push_action(current_block, action);
                }
            }
        }

        // The last block terminates with Return.
        bb.set_terminator(current_block, Terminator::Return);

        let returning = self.logical_plan.completion().var.get();

        let needs_transaction = self.use_transactions
            && bb
                .blocks
                .iter()
                .flat_map(|block| &block.actions)
                .filter(|a| a.is_db_op())
                .count()
                > 1;

        // Entry is always the first block (index 0).
        let entry = exec::BlockId::from_raw(0);

        ExecPlan {
            vars: VarStore::new(self.var_decls),
            blocks: bb.blocks,
            entry,
            returning,
            needs_transaction,
        }
    }
}
