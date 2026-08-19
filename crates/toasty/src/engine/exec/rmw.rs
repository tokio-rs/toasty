use crate::{
    Result,
    engine::{exec::Exec, mir},
};
use toasty_core::{
    driver::{
        ExecResponse, Rows,
        operation::{self, Transaction},
    },
    stmt,
};

impl Exec<'_> {
    pub(super) async fn exec_read_modify_write(
        &mut self,
        action: &mir::ReadModifyWrite,
    ) -> Result<ExecResponse> {
        assert!(action.inputs.is_empty(), "TODO");

        // When nested inside an outer transaction use savepoints so the outer
        // transaction can still commit or roll back as a whole. When standalone,
        // start our own transaction (MySQL requires an active BEGIN before
        // SAVEPOINT can be used, so we can't use savepoints here).
        let (begin, commit, rollback) = if self.in_transaction {
            let name = "read_modify_write";
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

        self.connection
            .exec(&self.engine.schema, begin.into())
            .await?;

        let rows = match self.rmw_exec(action).await {
            Ok(rows) => rows,
            Err(e) => {
                // Best effort: ignore rollback errors so the original error is returned
                let _ = self
                    .connection
                    .exec(&self.engine.schema, rollback.into())
                    .await;
                return Err(e);
            }
        };

        self.connection
            .exec(&self.engine.schema, commit.into())
            .await?;

        Ok(ExecResponse::from_rows(rows))
    }

    /// Execute the core read-then-write logic, returning the write's output
    /// rows on success. Errors on a DB failure or a condition mismatch;
    /// rollback (savepoint or transaction) is handled by the caller.
    ///
    /// The returned [`Rows`] mirrors what the equivalent unconditional write
    /// would produce: a `Count` when the write has no `RETURNING`, or a
    /// buffered row stream when it does. Rows are buffered before the caller
    /// commits so nothing is tied to the open transaction.
    async fn rmw_exec(&mut self, action: &mir::ReadModifyWrite) -> Result<Rows> {
        // Column types of the write's `RETURNING` rows, or `None` when the
        // write has no returning (it then reports only a row count).
        let output_ty = mir::row_field_types(&action.ty);

        // The probe projects the condition once per matched row.
        let ty = Some(vec![stmt::Type::Bool]);

        let mut read_stmt: stmt::Statement = action.read.clone().into();
        let read_params = self.engine.prepare_for_driver(&mut read_stmt);

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::QuerySql {
                    stmt: read_stmt,
                    params: read_params,
                    ret: ty,
                    last_insert_id: false,
                }
                .into(),
            )
            .await?;

        let Rows::Stream(rows) = res.values else {
            return Err(toasty_core::Error::invalid_result(
                "expected Stream, got Count",
            ));
        };

        // `matched` counts rows passing the filter; `satisfied` counts those
        // that also meet the condition. The write is safe to apply only when the
        // two agree — otherwise some matched row was concurrently advanced.
        let rows = rows.collect().await?;
        let matched = rows.len() as u64;
        let mut satisfied = 0;
        for row in &rows {
            let stmt::Value::Record(record) = row else {
                return Err(toasty_core::Error::invalid_result(
                    "conditional write probe expected Record",
                ));
            };
            match record.fields.first() {
                Some(stmt::Value::Bool(true)) => satisfied += 1,
                Some(stmt::Value::Bool(false)) | Some(stmt::Value::Null) => {}
                other => {
                    return Err(toasty_core::Error::invalid_result(format!(
                        "conditional write probe expected Bool, got {other:?}"
                    )));
                }
            }
        }

        // A conditional write targets a row the caller holds an instance of:
        // zero matched rows means it has since been deleted.
        if matched == 0 {
            return Err(toasty_core::Error::record_not_found(
                "conditional write matched no rows",
            ));
        }

        if matched != satisfied {
            return Err(toasty_core::Error::condition_failed(
                "write condition did not match",
            ));
        }

        let count = matched;
        let mut write_stmt = action.write.clone();

        // MySQL lacks `RETURNING` on `UPDATE`; strip the returning into a
        // follow-up `SELECT` that runs inside this same transaction. After this,
        // `write_stmt` carries a native `RETURNING` only on backends that
        // support it (SQLite, PostgreSQL).
        let mysql_update = self.process_stmt_update_with_returning_on_mysql(&mut write_stmt);
        let native_returning = write_stmt.returning().is_some();

        let write_params = self.engine.prepare_for_driver(&mut write_stmt);

        let res = self
            .connection
            .exec(
                &self.engine.schema,
                operation::QuerySql {
                    stmt: write_stmt,
                    params: write_params,
                    ret: if native_returning {
                        output_ty.clone()
                    } else {
                        None
                    },
                    last_insert_id: false,
                }
                .into(),
            )
            .await?;

        if let Some(mysql_update) = mysql_update {
            // The UPDATE ran without RETURNING; the captured SELECT produces the
            // post-update returning values.
            let mut res = self
                .run_mysql_update_returning_select(mysql_update, output_ty.clone())
                .await?;
            res.values.buffer().await?;
            return Ok(res.values);
        }

        if native_returning {
            // Buffer the returned rows before the caller commits so the stream
            // is not tied to the open transaction.
            let mut values = res.values;
            values.buffer().await?;
            return Ok(values);
        }

        let Rows::Count(actual) = res.values else {
            return Err(toasty_core::Error::invalid_result(
                "expected Count, got Stream",
            ));
        };

        // The write is supposed to touch exactly the probed rows. On backends
        // whose probe cannot lock (`select_for_update` is false), a concurrent
        // writer may change the matched set between the probe and the write —
        // surface a retryable conflict; the caller rolls back.
        if actual != count {
            return Err(toasty_core::Error::condition_failed(format!(
                "write applied to {actual} rows but the probe matched {count}"
            )));
        }

        Ok(Rows::Count(actual))
    }
}
