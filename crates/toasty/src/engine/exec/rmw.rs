use toasty_core::{
    driver::{
        operation::{self, Transaction},
        Rows,
    },
    stmt,
};

use super::{plan, Exec};

use crate::Result;

impl Exec<'_> {
    pub(super) async fn action_read_modify_write(
        &mut self,
        action: &plan::ReadModifyWrite,
    ) -> Result<()> {
        assert!(action.input.is_none(), "TODO");
        assert!(action.output.is_none(), "TODO");

        let res = self
            .db
            .driver
            .exec(&self.db.schema.db, Transaction::Start.into())
            .await?;
        assert!(matches!(res.rows, Rows::Count(0)));

        let ty = Some(vec![stmt::Type::I64, stmt::Type::I64]);

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
                operation::QuerySql {
                    stmt: action.read.clone().into(),
                    ret: ty,
                }
                .into(),
            )
            .await?;

        let Rows::Values(rows) = res.rows else {
            anyhow::bail!("expected rows");
        };

        let rows = rows.collect().await?;
        assert_eq!(rows.len(), 1);

        let stmt::Value::Record(record) = &rows[0] else {
            anyhow::bail!("expected record");
        };
        assert_eq!(record.len(), 2);

        let stmt::Value::I64(count) = record[0] else {
            anyhow::bail!("expected i64");
        };

        if record[0] != record[1] {
            anyhow::bail!("update condition did not match");
        }

        let res = self
            .db
            .driver
            .exec(
                &self.db.schema.db,
                operation::QuerySql {
                    stmt: action.write.clone(),
                    ret: None,
                }
                .into(),
            )
            .await?;

        let Rows::Count(actual) = res.rows else {
            anyhow::bail!("expected count");
        };

        assert_eq!(actual, count as u64);

        let res = self
            .db
            .driver
            .exec(&self.db.schema.db, Transaction::Commit.into())
            .await?;
        assert!(matches!(res.rows, Rows::Count(0)));

        Ok(())
    }
}
