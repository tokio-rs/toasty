mod execution;
mod nested_merge;
mod statement;

use crate::{
    Result,
    engine::{
        Engine, HirStatement,
        exec::{ExecPlan, Step, VarStore},
        hir,
        mir::{self, LogicalPlan},
    },
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
    steps: Vec<Step>,
    use_transactions: bool,
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
        let (steps, needs_transaction) = ExecPlanner {
            logical_plan: &logical_plan,
            steps: vec![],
            use_transactions: self.capability().sql(),
        }
        .plan_execution();

        ExecPlan {
            vars: VarStore::new(logical_plan.node_count(), self.schema.clone()),
            returning: logical_plan.completion(),
            plan: logical_plan,
            steps,
            needs_transaction,
        }
    }
}

impl HirPlanner<'_> {
    fn build_logical_plan(mut self) -> Result<mir::LogicalPlan> {
        // Reserve a data-load slot for every statement targeted by an effect
        // dep. An effect dep's target may be an ancestor planned after the
        // dependent (a statement-level cycle that is acyclic at the operation
        // level), so the anchor node must exist before planning starts; the
        // target's planning later fills the slot with an `Alias` to its
        // actual data-loading node.
        let hir = self.hir;
        for stmt_info in hir.statements() {
            for (&target, &kind) in &stmt_info.deps {
                if kind != hir::DepKind::Effect {
                    continue;
                }

                let slot = &hir[target].load_data_statement;
                if slot.get().is_none() {
                    slot.set(Some(self.mir.reserve()));
                }
            }
        }

        let root_id = self.hir.root_id();
        self.plan_statement(root_id)?;

        let exit = self.hir.root().output.get().unwrap();

        Ok(mir::LogicalPlan::new(self.mir, exit))
    }
}
