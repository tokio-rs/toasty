mod branch;
pub(crate) use branch::If;

mod delete_by_key;

mod eval;

mod exec_statement;
pub(crate) use exec_statement::{ConditionalOutput, PaginationConfig};

mod filter;

mod find_pk_by_index;

mod get_by_key;

mod kv;

mod nested_merge;
pub(crate) use nested_merge::{MergeIndex, MergeQualification, NestedChild, NestedLevel};

mod plan;
pub(crate) use plan::{ExecPlan, Step};

mod query_pk;

mod repeat;

mod rmw;

mod scan;

mod update_by_key;

mod upsert;

mod var;
pub(crate) use var::VarStore;

use crate::{
    Result,
    engine::{Engine, mir},
};
use toasty_core::{
    Connection,
    driver::{ExecResponse, Rows, operation::Transaction},
    stmt::{self, ValueStream},
};

struct Exec<'a> {
    engine: &'a Engine,
    connection: &'a mut dyn Connection,
    vars: VarStore,
    /// True when an outer transaction is active on this connection. Used by
    /// ReadModifyWrite to decide between savepoints (nested) and its own
    /// BEGIN/COMMIT (standalone).
    in_transaction: bool,
}

impl Engine {
    pub(crate) async fn exec_plan(
        &self,
        connection: &mut dyn Connection,
        plan: ExecPlan,
        in_transaction: bool,
    ) -> Result<ExecResponse> {
        let ExecPlan {
            plan: logical_plan,
            vars,
            steps,
            returning,
            needs_transaction,
        } = plan;

        let mut exec = Exec {
            engine: self,
            connection,
            vars,
            in_transaction,
        };

        // When nested inside an outer transaction use savepoints so the outer
        // transaction can still commit or roll back as a whole. When standalone,
        // start our own transaction (MySQL requires an active BEGIN before
        // SAVEPOINT can be used, so we can't use savepoints here).
        let (begin, commit, rollback) = if exec.in_transaction {
            let name = "statement";
            (
                Transaction::Savepoint(name.to_owned()),
                Transaction::ReleaseSavepoint(name.to_owned()),
                Transaction::RollbackToSavepoint(name.to_owned()),
            )
        } else {
            (
                Transaction::start(),
                Transaction::Commit,
                Transaction::Rollback,
            )
        };

        if needs_transaction {
            tracing::trace!("beginning plan transaction");
            exec.connection.exec(&self.schema, begin.into()).await?;
            exec.in_transaction = true;
        }

        for (i, step) in steps.iter().enumerate() {
            tracing::trace!(step = i, action = %step.name(&logical_plan), "executing action");
            // Debug, not error: the failure propagates to the caller, who
            // decides whether it is an application error. A handled unique
            // violation should not error-spam production logs.
            if let Err(e) = exec.exec_step(&logical_plan, step).await {
                tracing::debug!(step = i, action = %step.name(&logical_plan), error = %e, "action failed");
                if needs_transaction {
                    tracing::trace!("rolling back plan transaction due to error");
                    // Best effort: ignore rollback errors so the original error is returned
                    let _ = exec.connection.exec(&self.schema, rollback.into()).await;
                }
                return Err(e);
            }
        }

        if needs_transaction {
            tracing::trace!("committing plan transaction");
            exec.connection.exec(&self.schema, commit.into()).await?;
        }

        let response = exec.vars.load(returning).await?;
        tracing::trace!("final result from var {:?}:\n{:#?}", returning, response);

        // With exact use counts, the returning load was the last use of
        // the last live slot (success path only — the loop above returned
        // early on failure).
        exec.vars.assert_empty();

        let value_stream = match response.values {
            Rows::Count(_) => ValueStream::default(),
            Rows::Value(stmt::Value::List(items)) => ValueStream::from_vec(items),
            // TODO have the public API be able to handle single rows
            Rows::Value(value) => ValueStream::from_vec(vec![value]),
            Rows::Stream(value_stream) => value_stream,
        };

        Ok(ExecResponse {
            values: Rows::Stream(value_stream),
            next_cursor: response.next_cursor,
            prev_cursor: response.prev_cursor,
        })
    }
}

impl Exec<'_> {
    async fn exec_step(&mut self, logical_plan: &mir::LogicalPlan, step: &Step) -> Result<()> {
        match step {
            Step::Run(node_id) => self.exec_node(logical_plan, *node_id).await,
            Step::If(action) => self.action_if(logical_plan, action).await,
        }
    }

    /// Executes one node's operation and stores its output in the node's
    /// variable slot.
    async fn exec_node(
        &mut self,
        logical_plan: &mir::LogicalPlan,
        node_id: mir::NodeId,
    ) -> Result<()> {
        use mir::Operation;

        let node = &logical_plan[node_id];

        let response = match &node.op {
            // A pass-through: the response (stream included) relocates
            // between slots without buffering.
            Operation::Alias(op) => self.vars.load(op.input).await?,
            Operation::Const(op) => ExecResponse::from_rows(Rows::Value(op.value.clone())),
            Operation::DeleteByKey(op) => self.exec_delete_by_key(op).await?,
            Operation::Eval(op) => self.exec_eval(op).await?,
            Operation::ExecStatement(op) => self.exec_statement(op).await?,
            Operation::Filter(op) => self.exec_filter(op).await?,
            Operation::FindPkByIndex(op) => self.exec_find_pk_by_index(op).await?,
            Operation::GetByKey(op) => self.exec_get_by_key(op).await?,
            Operation::NestedMerge(op) => self.exec_nested_merge(op).await?,
            Operation::ReadModifyWrite(op) => self.exec_read_modify_write(op).await?,
            Operation::Repeat(op) => self.exec_repeat(op).await?,
            Operation::QueryPk(op) => self.exec_query_pk(op).await?,
            Operation::Scan(op) => self.exec_scan(op).await?,
            Operation::UpdateByKey(op) => self.exec_update_by_key(op).await?,
            Operation::Upsert(op) => self.exec_upsert(op).await?,
        };

        self.vars.store(node_id, node.ty(), node.num_uses, response);

        Ok(())
    }

    async fn collect_input(
        &mut self,
        input: impl IntoIterator<Item = mir::NodeId>,
    ) -> Result<Vec<stmt::Value>> {
        let mut ret = Vec::new();

        for node_id in input {
            let response = self.vars.load(node_id).await?;
            let value = response.values.collect_as_value().await?;
            ret.push(value);
        }

        Ok(ret)
    }
}
