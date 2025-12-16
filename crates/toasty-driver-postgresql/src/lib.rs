mod r#type;
mod value;

pub(crate) use value::Value;

use r#type::TypeExt;

use postgres::{tls::MakeTlsConnect, types::ToSql, Socket};
use std::sync::Arc;
use toasty_core::{
    driver::{Capability, Operation, Response},
    schema::db::{Schema, Table},
    stmt,
    stmt::ValueRecord,
    Connection, Result,
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

impl Connection for PostgreSQL {
    fn capability(&self) -> &'static Capability {
        &Capability::POSTGRESQL
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let (sql, ret_tys): (sql::Statement, _) = match op {
            Operation::Insert(op) => (op.stmt.into(), None),
            Operation::QuerySql(query) => (query.stmt.into(), query.ret),
            op => todo!("op={:#?}", op),
        };

        let width = sql.returning_len();

        let mut params = Vec::new();
        let sql_as_str = sql::Serializer::postgresql(schema).serialize(&sql, &mut params);

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();

        if width.is_none() {
            let args = params
                .iter()
                .map(|param| param as &(dyn ToSql + Sync))
                .collect::<Vec<_>>();
            let count = self.client.execute(&sql_as_str, &args).await?;
            return Ok(Response::count(count));
        }

        let args = params
            .iter()
            .map(|param| {
                (
                    param as &(dyn ToSql + Sync),
                    param.infer_ty().to_postgres_type(),
                )
            })
            .collect::<Vec<_>>();

        let rows = self.client.query_typed(&sql_as_str, &args).await?;

        if width.is_none() {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<usize, i64>(0);
            let condition_matched = row.get::<usize, i64>(1);

            if total == condition_matched {
                Ok(Response::count(total as _))
            } else {
                anyhow::bail!("update condition did not match");
            }
        } else {
            let ret_tys = ret_tys.as_ref().unwrap().clone();
            let results = rows.into_iter().map(move |row| {
                let mut results = Vec::new();
                for (i, column) in row.columns().iter().enumerate() {
                    results.push(Value::from_sql(i, &row, column, &ret_tys[i]).into_inner());
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(Response::value_stream(stmt::ValueStream::from_iter(
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
