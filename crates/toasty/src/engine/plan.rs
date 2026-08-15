mod execution;
mod nested_merge;
mod statement;

use crate::{
    Result,
    engine::{
        Engine, HirStatement,
        exec::{self, ExecPlan, VarDecls},
        hir,
        mir::{self, LogicalPlan},
    },
};
use std::sync::Arc;
use toasty_core::Schema;

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
    use_transactions: bool,
    schema: Arc<Schema>,
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
            use_transactions: self.capability().sql(),
            schema: self.schema.clone(),
        }
        .plan_execution()
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
        let exit_node = &self.mir[exit];

        // Increment num uses for the exit node. This counts as the "engines"
        // use of the variable to return to the use.
        exit_node.num_uses.set(exit_node.num_uses.get() + 1);

        Ok(mir::LogicalPlan::new(self.mir, exit))
    }
}
