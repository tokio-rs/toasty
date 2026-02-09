mod execution;
mod nested_merge;
mod statement;

use crate::{
    engine::{
        exec::{self, ExecPlan, VarDecls},
        mir::{self, LogicalPlan},
        Engine, HirStatement,
    },
    Result,
};

#[derive(Debug)]
struct HirPlanner<'a> {
    engine: &'a Engine,

    /// Root statement and all nested statements.
    hir: &'a HirStatement,

    /// Graph of operations needed to execute the statement
    mir: mir::Store,
}

#[derive(Debug)]
struct ExecPlanner<'a> {
    logical_plan: &'a LogicalPlan,
    var_decls: VarDecls,
    actions: Vec<exec::Action>,
}

impl Engine {
    pub(super) fn plan_hir_statement(&self, hir: HirStatement) -> Result<ExecPlan> {
        // Build the logical plan
        let logical_plan = HirPlanner {
            engine: self,
            hir: &hir,
            mir: mir::Store::new(),
        }
        .build_logical_plan()?;

        // Build the execution plan from the logical plan
        Ok(self.plan_execution(logical_plan))
    }

    fn plan_execution(&self, logical_plan: mir::LogicalPlan) -> ExecPlan {
        ExecPlanner {
            logical_plan: &logical_plan,
            var_decls: VarDecls::default(),
            actions: vec![],
        }
        .plan_execution()
    }
}

impl HirPlanner<'_> {
    fn build_logical_plan(mut self) -> Result<mir::LogicalPlan> {
        let root_id = self.hir.root_id();
        self.plan_statement(root_id)?;

        let exit = self.hir.root().output.get().unwrap();
        let exit_node = &self.mir.store[exit];

        // Increment num uses for the exit node. This counts as the "engines"
        // use of the variable to return to the use.
        exit_node.num_uses.set(exit_node.num_uses.get() + 1);

        Ok(mir::LogicalPlan::new(self.mir, exit))
    }
}
