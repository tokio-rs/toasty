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

        let res = self
            .connection
            .exec(&self.engine.schema.db, Transaction::Start.into())
            .await?;
        assert!(matches!(res.rows, Rows::Count(0)));

        let ty = Some(vec![stmt::Type::I64, stmt::Type::I64]);

        let res = self
            .connection
            .exec(
                &self.engine.schema.db,
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
                &self.engine.schema.db,
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

        let res = self
            .connection
            .exec(&self.engine.schema.db, Transaction::Commit.into())
            .await?;
        assert!(matches!(res.rows, Rows::Count(0)));

        if let Some(output) = &action.output {
            let rows = Rows::value_stream(ValueStream::default());
            self.vars.store(output.var, output.num_uses, rows);
        }

        Ok(())
    }
}

impl From<ReadModifyWrite> for Action {
    fn from(value: ReadModifyWrite) -> Self {
        Self::ReadModifyWrite(Box::new(value))
    }
}
