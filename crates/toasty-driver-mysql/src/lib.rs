mod value;
pub(crate) use value::Value;

use std::sync::Arc;

use mysql_async::{
    consts::ColumnType::{
        MYSQL_TYPE_INT24, MYSQL_TYPE_LONG, MYSQL_TYPE_LONGLONG, MYSQL_TYPE_NULL, MYSQL_TYPE_SHORT,
        MYSQL_TYPE_TINY, MYSQL_TYPE_VARCHAR,
    },
    prelude::{Queryable, ToValue},
    Pool,
};
use toasty_core::{
    driver::{self, Capability, Operation, Response},
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

        let pool = Pool::from_url(url.as_ref())?;
        Ok(Self { pool })
    }

    pub async fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let mut params = Vec::new();

        let sql = serializer.serialize(&sql::Statement::create_table(table), &mut params);

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
        println!("se dropeo");
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
    fn capability(&self) -> &Capability {
        &Capability::Sql(driver::CapabilitySql {
            cte_with_update: true,
        })
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let mut conn = self.pool.get_conn().await?;

        let sql: sql::Statement = match op {
            Operation::Insert(stmt) => stmt.into(),
            Operation::QuerySql(query) => query.stmt.into(),
            op => todo!("op={:#?}", op),
        };

        let conditional_update =
            matches!(&sql, sql::Statement::Update(stmt) if stmt.condition.is_some());

        let width = sql.returning_len();

        let mut params = Vec::new();

        let sql_as_str = sql::Serializer::mysql(schema).serialize(&sql, &mut params);

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();
        let args = params
            .iter()
            .map(|param| param.to_value())
            .collect::<Vec<_>>();

        if width.is_none() && !conditional_update {
            let count = conn
                .exec::<mysql_async::Row, &String, mysql_async::Params>(
                    &sql_as_str,
                    mysql_async::Params::Positional(args),
                )
                .await?
                .len();

            return Ok(Response::from_count(count));
        }

        let rows: Vec<mysql_async::Row> = conn.exec(&sql_as_str, &args).await?;

        if width.is_none() {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<i64, usize>(0).unwrap();
            let condition_matched = row.get::<i64, usize>(1).unwrap();

            if total == condition_matched {
                Ok(Response::from_count(total as _))
            } else {
                anyhow::bail!("update condition did not match");
            }
        } else {
            let results = rows.into_iter().map(move |row| {
                let mut results = Vec::new();
                for mut i in 0..row.len() {
                    if conditional_update {
                        i += 2;
                    }

                    let column = &row.columns()[i];
                    results.push(mysql_to_toasty(i, &row, column));
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(Response::from_value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        }
    }

    // TODO: Check the boolean from postgress impl
    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        println!("vamooo");
        for table in &schema.tables {
            self.drop_table(schema, table, true).await?;
            self.create_table(schema, table).await?;
        }

        Ok(())
    }
}

fn mysql_to_toasty(i: usize, row: &mysql_async::Row, column: &mysql_async::Column) -> stmt::Value {
    match column.column_type() {
        MYSQL_TYPE_NULL => stmt::Value::Null,
        MYSQL_TYPE_VARCHAR => row
            .get(i)
            .map(stmt::Value::String)
            .unwrap_or(stmt::Value::Null),
        MYSQL_TYPE_TINY => {
            if column.column_length() == 1 {
                row.get(i)
                    .map(stmt::Value::Bool)
                    .unwrap_or(stmt::Value::Null)
            } else {
                row.get(i)
                    .map(stmt::Value::I64)
                    .unwrap_or(stmt::Value::Null)
            }
        }
        MYSQL_TYPE_SHORT | MYSQL_TYPE_INT24 | MYSQL_TYPE_LONG | MYSQL_TYPE_LONGLONG => {
            row.get::<i64, usize>(i)
                .map(stmt::Value::I64)
                .unwrap_or(stmt::Value::Null);
            todo!()
        }
        _ => todo!(
            "implement PostgreSQL to toasty conversion for `{:#?}`",
            column.column_type()
        ),
    }
}
