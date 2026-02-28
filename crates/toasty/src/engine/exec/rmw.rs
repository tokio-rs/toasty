use crate::{
    engine::exec::{Action, Exec, Output, VarId},
    Result,
};
use toasty_core::{
    driver::{
        operation::{self, Transaction},
        Rows,
    },
    stmt::{self, ValueStream},
};

#[derive(Debug)]
pub(crate) struct ReadModifyWrite {
    /// Where to get arguments for this action.
    pub input: Vec<VarId>,

    /// How to handle output
    pub output: Option<Output>,

    /// Read statement
    pub read: stmt::Query,

    /// Write statement
    pub write: stmt::Statement,
}

impl Exec<'_> {
    pub(super) async fn action_read_modify_write(
        &mut self,
        action: &ReadModifyWrite,
    ) -> Result<()> {
        assert!(action.input.is_empty(), "TODO");

        // When nested inside an outer transaction use savepoints so the outer
        // transaction can still commit or roll back as a whole. When standalone,
        // start our own transaction (MySQL requires an active BEGIN before
        // SAVEPOINT can be used, so we can't use savepoints here).
        let (begin, commit, rollback) = if self.in_transaction {
            let id = self.generate_savepoint_id();
            (
                Transaction::Savepoint(id),
                Transaction::ReleaseSavepoint(id),
                Transaction::RollbackToSavepoint(id),
            )
        } else {
            (
                Transaction::start(),
                Transaction::Commit,
                Transaction::Rollback,
            )
        };

        self.connection
            .exec(&self.engine.schema, begin.into())
            .await?;

        if let Err(e) = self.rmw_exec(action).await {
            // Best effort: ignore rollback errors so the original error is returned
            let _ = self
                .connection
                .exec(&self.engine.schema, rollback.into())
                .await;
            return Err(e);
        }

        self.connection
            .exec(&self.engine.schema, commit.into())
            .await?;

        if let Some(output) = &action.output {
            let rows = Rows::value_stream(ValueStream::default());
            self.vars.store(output.var, output.num_uses, rows);
        }

        Ok(())
    }

    /// Execute the core read-then-write logic, returning an error on either a
    /// DB failure or a condition mismatch. Rollback (savepoint or transaction)
    /// is handled by the caller.
    async fn rmw_exec(&mut self, action: &ReadModifyWrite) -> Result<()> {
        let ty = Some(vec![stmt::Type::I64, stmt::Type::I64]);

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::QuerySql {
                    stmt: action.read.clone().into(),
                    ret: ty,
                    last_insert_id_hack: None,
                }
                .into(),
            )
            .await?;

        let Rows::Stream(rows) = res.rows else {
            return Err(toasty_core::Error::invalid_result(
                "expected Stream, got Count",
            ));
        };

        let rows = rows.collect().await?;
        assert_eq!(rows.len(), 1);

        let stmt::Value::Record(record) = &rows[0] else {
            return Err(toasty_core::Error::invalid_result("expected Record value"));
        };
        assert_eq!(record.len(), 2);

        let stmt::Value::I64(count) = record[0] else {
            return Err(toasty_core::Error::invalid_result("expected I64 value"));
        };

        if record[0] != record[1] {
            return Err(toasty_core::Error::condition_failed(
                "update condition did not match",
            ));
        }

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::QuerySql {
                    stmt: action.write.clone(),
                    ret: None,
                    last_insert_id_hack: None,
                }
                .into(),
            )
            .await?;

        let Rows::Count(actual) = res.rows else {
            return Err(toasty_core::Error::invalid_result(
                "expected Count, got Stream",
            ));
        };

        assert_eq!(actual, count as u64);

        Ok(())
    }
}

impl From<ReadModifyWrite> for Action {
    fn from(value: ReadModifyWrite) -> Self {
        Self::ReadModifyWrite(Box::new(value))
    }
}
