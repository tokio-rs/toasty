mod statement_cache;
mod r#type;
mod value;

pub(crate) use value::Value;

use postgres::{tls::MakeTlsConnect, types::ToSql, Socket};
use std::{borrow::Cow, sync::Arc};
use toasty_core::{
    async_trait,
    driver::{Capability, Driver, Operation, Response},
    schema::db::{Migration, Schema, SchemaDiff, Table},
    stmt,
    stmt::ValueRecord,
    Result,
};
use toasty_sql::{self as sql, TypedValue};
use tokio_postgres::{Client, Config};
use url::Url;

use crate::{r#type::TypeExt, statement_cache::StatementCache};

#[derive(Debug)]
pub struct PostgreSQL {
    url: String,
    config: Config,
}

impl PostgreSQL {
    /// Create a new PostgreSQL driver from a connection URL
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = Url::parse(&url_str).map_err(toasty_core::Error::driver_operation_failed)?;

        if !matches!(url.scheme(), "postgresql" | "postgres") {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection URL does not have a `postgresql` scheme; url={}",
                url
            )));
        }

        let host = url.host_str().ok_or_else(|| {
            toasty_core::Error::invalid_connection_url(format!(
                "missing host in connection URL; url={}",
                url
            ))
        })?;

        if url.path().is_empty() {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "no database specified - missing path in connection URL; url={}",
                url
            )));
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

        Ok(Self {
            url: url_str,
            config,
        })
    }
}

#[async_trait]
impl Driver for PostgreSQL {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.url)
    }

    fn capability(&self) -> &'static Capability {
        &Capability::POSTGRESQL
    }

    async fn connect(&self) -> toasty_core::Result<Box<dyn toasty_core::driver::Connection>> {
        Ok(Box::new(
            Connection::connect(self.config.clone(), tokio_postgres::NoTls).await?,
        ))
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::POSTGRESQL);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| {
                let mut params = Vec::<TypedValue>::new();
                let sql = sql::Serializer::postgresql(stmt.schema())
                    .serialize(stmt.statement(), &mut params);
                assert!(
                    params.is_empty(),
                    "migration statements should not have parameters"
                );
                sql
            })
            .collect();

        Migration::new_sql(sql_strings.join("\n"))
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        let dbname = self
            .config
            .get_dbname()
            .ok_or_else(|| {
                toasty_core::Error::invalid_connection_url("no database name configured")
            })?
            .to_string();

        // We cannot drop a database we are currently connected to, so we need a temp database.
        let temp_dbname = "__toasty_reset_temp";

        let connect = |dbname: &str| {
            let mut config = self.config.clone();
            config.dbname(dbname);
            Connection::connect(config, tokio_postgres::NoTls)
        };

        // Step 1: Connect to the target DB and create a temp DB
        let conn = connect(&dbname).await?;
        conn.client
            .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", temp_dbname), &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        conn.client
            .execute(&format!("CREATE DATABASE \"{}\"", temp_dbname), &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        drop(conn);

        // Step 2: Connect to the temp DB, drop and recreate the target
        let conn = connect(temp_dbname).await?;
        conn.client
            .execute(
                "SELECT pg_terminate_backend(pid) \
                 FROM pg_stat_activity \
                 WHERE datname = $1 AND pid <> pg_backend_pid()",
                &[&dbname],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        conn.client
            .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", dbname), &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        conn.client
            .execute(&format!("CREATE DATABASE \"{}\"", dbname), &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        drop(conn);

        // Step 3: Connect back to the target and clean up the temp DB
        let conn = connect(&dbname).await?;
        conn.client
            .execute(&format!("DROP DATABASE IF EXISTS \"{}\"", temp_dbname), &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Connection {
    client: Client,
    statement_cache: StatementCache,
}

impl Connection {
    /// Initialize a Toasty PostgreSQL connection using an initialized client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            statement_cache: StatementCache::new(100),
        }
    }

    /// Connects to a PostgreSQL database using a [`postgres::Config`].
    ///
    /// See [`postgres::Client::configure`] for more information.
    pub async fn connect<T>(config: Config, tls: T) -> Result<Self>
    where
        T: MakeTlsConnect<Socket> + 'static,
        T::Stream: Send,
    {
        let (client, connection) = config
            .connect(tls)
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {e}");
            }
        });

        Ok(Self::new(client))
    }

    /// Creates a table.
    pub async fn create_table(&mut self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::postgresql(schema);

        let mut params: Vec<toasty_sql::TypedValue> = Vec::new();
        let sql = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::POSTGRESQL),
            &mut params,
        );

        assert!(
            params.is_empty(),
            "creating a table shouldn't involve any parameters"
        );

        self.client
            .execute(&sql, &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

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

            self.client
                .execute(&sql, &[])
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
        }

        Ok(())
    }
}

impl From<Client> for Connection {
    fn from(client: Client) -> Self {
        Self {
            client,
            statement_cache: StatementCache::new(100),
        }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let (sql, ret_tys): (sql::Statement, _) = match op {
            Operation::Insert(op) => (op.stmt.into(), None),
            Operation::QuerySql(query) => {
                assert!(
                    query.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for PostgreSQL"
                );
                (query.stmt.into(), query.ret)
            }
            op => todo!("op={:#?}", op),
        };

        let width = sql.returning_len();

        let mut params: Vec<toasty_sql::TypedValue> = Vec::new();
        let sql_as_str = sql::Serializer::postgresql(schema).serialize(&sql, &mut params);

        let param_types = params
            .iter()
            .map(|typed_value| typed_value.infer_ty().to_postgres_type())
            .collect::<Vec<_>>();

        let values: Vec<_> = params.into_iter().map(|tv| Value::from(tv.value)).collect();
        let params = values
            .iter()
            .map(|param| param as &(dyn ToSql + Sync))
            .collect::<Vec<_>>();

        let statement = self
            .statement_cache
            .prepare_typed(&mut self.client, &sql_as_str, &param_types)
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        if width.is_none() {
            let count = self
                .client
                .execute(&statement, &params)
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
            return Ok(Response::count(count));
        }

        let rows = self
            .client
            .query(&statement, &params)
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        if width.is_none() {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<usize, i64>(0);
            let condition_matched = row.get::<usize, i64>(1);

            if total == condition_matched {
                Ok(Response::count(total as _))
            } else {
                Err(toasty_core::Error::condition_failed(
                    "update condition did not match",
                ))
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

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.create_table(schema, table).await?;
        }
        Ok(())
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        // Ensure the migrations table exists
        self.client
            .execute(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
                &[],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Query all applied migrations
        let rows = self
            .client
            .query(
                "SELECT id FROM __toasty_migrations ORDER BY applied_at",
                &[],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        Ok(rows
            .iter()
            .map(|row| {
                let id: i64 = row.get(0);
                toasty_core::schema::db::AppliedMigration::new(id as u64)
            })
            .collect())
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: String,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        // Ensure the migrations table exists
        self.client
            .execute(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id BIGINT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TIMESTAMP NOT NULL
            )",
                &[],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Start transaction
        let transaction = self
            .client
            .transaction()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Execute each migration statement
        for statement in migration.statements() {
            if let Err(e) = transaction
                .batch_execute(statement)
                .await
                .map_err(toasty_core::Error::driver_operation_failed)
            {
                transaction
                    .rollback()
                    .await
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Err(e);
            }
        }

        // Record the migration
        if let Err(e) = transaction
            .execute(
                "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES ($1, $2, NOW())",
                &[&(id as i64), &name],
            )
            .await
            .map_err(toasty_core::Error::driver_operation_failed)
        {
            transaction
                .rollback()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
            return Err(e);
        }

        // Commit transaction
        transaction
            .commit()
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;
        Ok(())
    }
}
