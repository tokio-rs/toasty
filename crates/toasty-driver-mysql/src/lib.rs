#![allow(clippy::needless_range_loop)]

mod value;
pub(crate) use value::Value;

use mysql_async::{
    prelude::{Queryable, ToValue},
    Pool,
};
use std::sync::Arc;
use toasty_core::{
    driver::{operation::Transaction, Capability, Operation, Response},
    schema::db::{Schema, Table},
    stmt::{self, ValueRecord},
    Driver, Result,
};
use toasty_sql as sql;
use url::Url;

#[derive(Debug)]
pub struct MySQL {
    pool: Pool,
}

impl MySQL {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "mysql" {
            return Err(anyhow::anyhow!(
                "connection url does not have a `mysql` scheme; url={}",
                url
            ));
        }

        url.host_str()
            .ok_or_else(|| anyhow::anyhow!("missing host in connection URL; url={}", url))?;

        if url.path().is_empty() {
            return Err(anyhow::anyhow!(
                "no database specified - missing path in connection URL; url={}",
                url
            ));
        }

        let opts = mysql_async::Opts::from_url(url.as_ref())?;
        let opts = mysql_async::OptsBuilder::from_opts(opts).client_found_rows(true);

        let pool = Pool::new(opts);
        Ok(Self { pool })
    }

    pub async fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let mut params = Vec::new();

        let sql = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::MYSQL),
            &mut params,
        );

        assert!(
            params.is_empty(),
            "creating a table shouldn't involve any parameters"
        );

        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(&sql, ()).await?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index), &mut params);

            assert!(
                params.is_empty(),
                "creating an index shouldn't involve any parameters"
            );

            conn.exec_drop(&sql, ()).await?;
        }

        Ok(())
    }

    /// Drops a table.
    pub async fn drop_table(&self, schema: &Schema, table: &Table, if_exists: bool) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);
        let mut params = Vec::new();

        let sql = if if_exists {
            serializer.serialize(&sql::Statement::drop_table_if_exists(table), &mut params)
        } else {
            serializer.serialize(&sql::Statement::drop_table(table), &mut params)
        };

        assert!(
            params.is_empty(),
            "dropping a table shouldn't involve any parameters"
        );

        let mut conn = self.pool.get_conn().await?;

        conn.exec_drop(&sql, ()).await?;
        Ok(())
    }
}
impl From<Pool> for MySQL {
    fn from(pool: Pool) -> Self {
        Self { pool }
    }
}

#[toasty_core::async_trait]
impl Driver for MySQL {
    fn capability(&self) -> &'static Capability {
        &Capability::MYSQL
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let mut conn = self.pool.get_conn().await?;

        let (sql, ret): (sql::Statement, _) = match op {
            // Operation::Insert(stmt) => stmt.into(),
            Operation::QuerySql(op) => (op.stmt.into(), op.ret),
            Operation::Transaction(Transaction::Start) => {
                conn.query_drop("START TRANSACTION").await?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Commit) => {
                conn.query_drop("COMMIT").await?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Rollback) => {
                conn.query_drop("ROLLBACK").await?;
                return Ok(Response::count(0));
            }
            op => todo!("op={:#?}", op),
        };

        let mut params = Vec::new();

        let sql_as_str = sql::Serializer::mysql(schema).serialize(&sql, &mut params);

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();
        let args = params
            .iter()
            .map(|param| param.to_value())
            .collect::<Vec<_>>();

        if ret.is_none() {
            let count = conn
                .exec_iter(&sql_as_str, mysql_async::Params::Positional(args))
                .await?
                .affected_rows();

            return Ok(Response::count(count));
        }

        let rows: Vec<mysql_async::Row> = conn.exec(&sql_as_str, &args).await?;

        if let Some(returning) = ret {
            let results = rows.into_iter().map(move |mut row| {
                assert_eq!(
                    row.len(),
                    returning.len(),
                    "row={row:#?}; returning={returning:#?}"
                );

                let mut results = Vec::new();
                for i in 0..row.len() {
                    let column = &row.columns()[i];
                    results.push(Value::from_sql(i, &mut row, column, &returning[i]).into_inner());
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(Response::value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        } else {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<i64, usize>(0).unwrap();
            let condition_matched = row.get::<i64, usize>(1).unwrap();

            if total == condition_matched {
                Ok(Response::count(total as _))
            } else {
                anyhow::bail!("update condition did not match");
            }
        }
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.drop_table(schema, table, true).await?;
            self.create_table(schema, table).await?;
        }

        Ok(())
    }
}
