mod value;
pub(crate) use value::Value;

use std::sync::Arc;

use postgres::{
    tls::MakeTlsConnect,
    types::{ToSql, Type},
    Column, Row, Socket,
};
use toasty_core::{
    driver::{Capability, Operation, Response},
    schema::db::{Schema, Table},
    stmt,
    stmt::ValueRecord,
    Driver, Result,
};
use toasty_sql as sql;
use tokio_postgres::{Client, Config};
use url::Url;

#[derive(Debug)]
pub struct PostgreSQL {
    /// The PostgreSQL client.
    client: Client,
}

impl PostgreSQL {
    /// Initialize a Toasty PostgreSQL driver using an initialized connection.
    pub fn new(connection: Client) -> Self {
        Self { client: connection }
    }

    /// Connects to a PostgreSQL database using a connection string.
    ///
    /// See [`postgres::Client::connect`] for more information.
    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "postgresql" {
            return Err(anyhow::anyhow!(
                "connection URL does not have a `postgresql` scheme; url={}",
                url
            ));
        }

        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("missing host in connection URL; url={}", url))?;

        if url.path().is_empty() {
            return Err(anyhow::anyhow!(
                "no database specified - missing path in connection URL; url={}",
                url
            ));
        }

        let mut config = Config::new();
        config.host(host);
        config.dbname(url.path().trim_start_matches('/'));

        if let Some(port) = url.port() {
            config.port(port);
        }

        if !url.username().is_empty() {
            config.user(url.username());
        }

        if let Some(password) = url.password() {
            config.password(password);
        }

        Self::connect_with_config(config, tokio_postgres::NoTls).await
    }

    /// Connects to a PostgreSQL database using a [`postgres::Config`].
    ///
    /// See [`postgres::Client::configure`] for more information.
    pub async fn connect_with_config<T>(config: Config, tls: T) -> Result<Self>
    where
        T: MakeTlsConnect<Socket> + 'static,
        T::Stream: Send,
    {
        let (client, connection) = config.connect(tls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {e}");
            }
        });

        Ok(Self::new(client))
    }

    /// Creates a table.
    pub async fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::postgresql(schema);

        let mut params = Vec::new();
        let sql = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::POSTGRESQL),
            &mut params,
        );

        assert!(
            params.is_empty(),
            "creating a table shouldn't involve any parameters"
        );

        self.client.execute(&sql, &[]).await?;

        // NOTE: `params` is guaranteed to be empty based on the assertion above. If
        // that changes, `params.clear()` should be called here.
        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index), &mut params);

            assert!(
                params.is_empty(),
                "creating an index shouldn't involve any parameters"
            );

            self.client.execute(&sql, &[]).await?;
        }

        Ok(())
    }

    /// Drops a table.
    pub async fn drop_table(&self, schema: &Schema, table: &Table, if_exists: bool) -> Result<()> {
        let serializer = sql::Serializer::postgresql(schema);
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

        self.client.execute(&sql, &[]).await?;
        Ok(())
    }
}

impl From<Client> for PostgreSQL {
    fn from(client: Client) -> Self {
        Self { client }
    }
}

#[toasty_core::async_trait]
impl Driver for PostgreSQL {
    fn capability(&self) -> &Capability {
        &Capability::POSTGRESQL
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let sql: sql::Statement = match op {
            Operation::Insert(op) => op.stmt.into(),
            Operation::QuerySql(query) => query.stmt.into(),
            op => todo!("op={:#?}", op),
        };

        let width = sql.returning_len();

        let mut params = Vec::new();
        let sql_as_str = sql::Serializer::postgresql(schema).serialize(&sql, &mut params);

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();

        let args = params
            .iter()
            .map(|param| param as &(dyn ToSql + Sync))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        if width.is_none() {
            let count = self.client.execute(&sql_as_str, &args).await?;
            return Ok(Response::from_count(count));
        }

        let rows = self.client.query(&sql_as_str, &args).await?;

        if width.is_none() {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<usize, i64>(0);
            let condition_matched = row.get::<usize, i64>(1);

            if total == condition_matched {
                Ok(Response::from_count(total as _))
            } else {
                anyhow::bail!("update condition did not match");
            }
        } else {
            let results = rows.into_iter().map(move |row| {
                let mut results = Vec::new();
                for i in 0..row.len() {
                    let column = &row.columns()[i];
                    results.push(postgres_to_toasty(i, &row, column));
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(Response::from_value_stream(stmt::ValueStream::from_iter(
                results,
            )))
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

/// Converts a PostgreSQL value within a row to a [`toasty_core::stmt::Value`].
fn postgres_to_toasty(index: usize, row: &Row, column: &Column) -> stmt::Value {
    // NOTE: unfortunately, the inner representation of the PostgreSQL type enum is not
    // accessible, so we must manually match each type like so.
    if column.type_() == &Type::TEXT || column.type_() == &Type::VARCHAR {
        row.get::<usize, Option<String>>(index)
            .map(stmt::Value::String)
            .unwrap_or(stmt::Value::Null)
    } else if column.type_() == &Type::BOOL {
        row.get::<usize, Option<bool>>(index)
            .map(stmt::Value::Bool)
            .unwrap_or(stmt::Value::Null)
    } else if column.type_() == &Type::INT4 {
        row.get::<usize, Option<i32>>(index)
            .map(stmt::Value::I32)
            .unwrap_or(stmt::Value::Null)
    } else if column.type_() == &Type::INT8 {
        row.get::<usize, Option<i64>>(index)
            .map(stmt::Value::from)
            .unwrap_or(stmt::Value::Null)
    } else {
        todo!(
            "implement PostgreSQL to toasty conversion for `{:#?}`",
            column.type_()
        );
    }
}
