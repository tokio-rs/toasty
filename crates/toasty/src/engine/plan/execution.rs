use crate::engine::{
    exec::{Action, Atomicity, ExecPlan, VarStore},
    plan::ExecPlanner,
};
use toasty_core::driver::Capability;

impl ExecPlanner<'_> {
    pub(super) fn plan_execution(mut self) -> crate::Result<ExecPlan> {
        // Convert each node in execution order
        for node in self.logical_plan.operations() {
            let action = node.to_exec(self.logical_plan, &mut self.var_decls);
            self.actions.push(action);
        }

        let returning = self.logical_plan.completion().var.get();

        let atomicity = plan_atomicity(self.capability, &self.actions)?;

        Ok(ExecPlan {
            vars: VarStore::new(self.var_decls, self.schema),
            actions: self.actions,
            returning,
            atomicity,
        })
    }
}

fn plan_atomicity(capability: &Capability, actions: &[Action]) -> crate::Result<Atomicity> {
    let requires_interactive = actions.iter().any(Action::requires_interactive_transaction);
    let database_operations = actions.iter().filter(|action| action.is_db_op()).count();
    let atomicity = select_atomicity(capability, database_operations, requires_interactive)?;
    if atomicity == Atomicity::AtomicBatch
        && actions
            .iter()
            .any(|action| !action.is_atomic_batch_eligible())
    {
        return Err(toasty_core::Error::unsupported_feature(format!(
            "{} cannot atomically batch a plan with database result dependencies or non-generated SQL operations",
            capability.driver_name
        )));
    }
    Ok(atomicity)
}

fn select_atomicity(
    capability: &Capability,
    database_operations: usize,
    requires_interactive: bool,
) -> crate::Result<Atomicity> {
    if requires_interactive && !capability.interactive_transactions {
        return Err(toasty_core::Error::unsupported_feature(format!(
            "{} does not support read-modify-write operations because they require interactive transactions",
            capability.driver_name
        )));
    }

    if database_operations <= 1 || !capability.sql {
        return Ok(Atomicity::None);
    }

    if capability.interactive_transactions {
        Ok(Atomicity::InteractiveTransaction)
    } else if capability.atomic_batch {
        Ok(Atomicity::AtomicBatch)
    } else {
        Err(toasty_core::Error::unsupported_feature(format!(
            "{} cannot execute a {database_operations}-operation plan atomically",
            capability.driver_name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_operation_needs_no_atomic_wrapper() {
        assert_eq!(
            select_atomicity(&Capability::SQLITE, 1, false).unwrap(),
            Atomicity::None
        );
    }

    #[test]
    fn existing_sql_drivers_use_interactive_transactions() {
        assert_eq!(
            select_atomicity(&Capability::POSTGRESQL, 2, false).unwrap(),
            Atomicity::InteractiveTransaction
        );
    }

    #[test]
    fn d1_multi_operation_plans_require_atomic_batches() {
        assert_eq!(
            select_atomicity(&Capability::D1, 2, false).unwrap(),
            Atomicity::AtomicBatch
        );
    }

    #[test]
    fn d1_rejects_read_modify_write_plans() {
        let error = select_atomicity(&Capability::D1, 1, true).unwrap_err();
        assert!(error.is_unsupported_feature());
    }

    #[test]
    fn sql_driver_without_atomic_mechanism_is_rejected() {
        let capability = Capability {
            interactive_transactions: false,
            atomic_batch: false,
            transaction_lock_mode: false,
            ..Capability::SQLITE
        };
        let error = select_atomicity(&capability, 2, false).unwrap_err();
        assert!(error.is_unsupported_feature());
    }
}
