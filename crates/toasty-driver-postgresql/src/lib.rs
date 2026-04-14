#![warn(missing_docs)]

//! Toasty driver for [PostgreSQL](https://www.postgresql.org/) using
//! [`tokio-postgres`](https://docs.rs/tokio-postgres).
//!
//! # Examples
//!
//! ```no_run
//! use toasty_driver_postgresql::PostgreSQL;
//!
//! let driver = PostgreSQL::new("postgresql://localhost/mydb").unwrap();
//! ```

mod statement_cache;
#[cfg(feature = "tls")]
mod tls;
mod r#type;
mod value;

pub(crate) use value::Value;

use async_trait::async_trait;
use percent_encoding::percent_decode_str;
use std::{borrow::Cow, sync::Arc};
use toasty_core::{
    Result, Schema,
    driver::{Capability, Driver, ExecResponse, Operation, operation},
    schema::db::{self, Migration, SchemaDiff, Table},
    stmt,
    stmt::ValueRecord,
};
use toasty_sql::{self as sql};
use tokio_postgres::{Client, Config, Socket, tls::MakeTlsConnect, types::ToSql};
use url::Url;

use crate::{statement_cache::StatementCache, r#type::TypeExt};

/// A PostgreSQL [`Driver`] that connects via `tokio-postgres`.
///
/// # Examples
///
/// ```no_run
/// use toasty_driver_postgresql::PostgreSQL;
///
/// let driver = PostgreSQL::new("postgresql://localhost/mydb").unwrap();
/// ```
#[derive(Debug)]
pub struct PostgreSQL {
    url: String,
    config: Config,
    #[cfg(feature = "tls")]
    tls: Option<tls::MakeRustlsConnect>,
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

        let dbname = percent_decode_str(url.path().trim_start_matches('/'))
            .decode_utf8()
            .map_err(|_| {
                toasty_core::Error::invalid_connection_url("database name is not valid UTF-8")
            })?;
        config.dbname(&*dbname);

        if let Some(port) = url.port() {
            config.port(port);
        }

        if !url.username().is_empty() {
            let user = percent_decode_str(url.username())
                .decode_utf8()
                .map_err(|_| {
                    toasty_core::Error::invalid_connection_url("username is not valid UTF-8")
                })?;
            config.user(&*user);
        }

        if let Some(password) = url.password() {
            config.password(percent_decode_str(password).collect::<Vec<u8>>());
        }

        #[cfg(feature = "tls")]
        let tls = tls::configure_tls(&url, &mut config)?;

        #[cfg(not(feature = "tls"))]
        for (key, value) in url.query_pairs() {
            if key == "sslmode" && value != "disable" {
                return Err(toasty_core::Error::invalid_connection_url(
                    "TLS not available: compile with the `tls` feature",
                ));
            }
        }

        Ok(Self {
            url: url_str,
            config,
            #[cfg(feature = "tls")]
            tls,
        })
    }

    async fn connect_with_config(&self, config: Config) -> Result<Connection> {
        #[cfg(feature = "tls")]
        if let Some(ref tls) = self.tls {
            return Connection::connect(config, tls.clone()).await;
        }
        Connection::connect(config, tokio_postgres::NoTls).await
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
            self.connect_with_config(self.config.clone()).await?,
        ))
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::POSTGRESQL);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::postgresql(stmt.schema()).serialize(stmt.statement()))
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
            self.connect_with_config(config)
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

/// An open connection to a PostgreSQL database.
#[derive(Debug)]
pub struct Connection {
    client: Client,
    statement_cache: StatementCache,
    /// Cached PostgreSQL `Type` objects for native enum types, keyed by type name.
    /// Cached PostgreSQL `Type` objects for native enum types, keyed by type name.
    /// Lazily populated by querying `pg_type` on first use.
    enum_types: std::collections::HashMap<String, tokio_postgres::types::Type>,
}

impl Connection {
    /// Initialize a Toasty PostgreSQL connection using an initialized client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            statement_cache: StatementCache::new(100),
            enum_types: std::collections::HashMap::new(),
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

    /// Resolve a `db::Type` to a PostgreSQL wire type. For native enum types,
    /// lazily queries `pg_type` for the OID and caches the result.
    async fn resolve_param_type(
        &mut self,
        ty: &toasty_core::schema::db::Type,
    ) -> Result<tokio_postgres::types::Type> {
        if let toasty_core::schema::db::Type::Enum(type_enum) = ty
            && let Some(name) = &type_enum.name
        {
            // Check cache first
            if let Some(pg_type) = self.enum_types.get(name) {
                return Ok(pg_type.clone());
            }

            // Query pg_type for the OID
            let oid_row = self
                .client
                .query_one("SELECT oid FROM pg_type WHERE typname = $1", &[name])
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;
            let oid: u32 = oid_row.get(0);
            let variants: Vec<String> = type_enum.variants.iter().map(|v| v.name.clone()).collect();
            let pg_type = tokio_postgres::types::Type::new(
                name.clone(),
                oid,
                tokio_postgres::types::Kind::Enum(variants),
                "public".to_string(),
            );
            self.enum_types.insert(name.clone(), pg_type.clone());
            return Ok(pg_type);
        }

        Ok(ty.to_postgres_type())
    }

    /// Creates a table.
    pub async fn create_table(&mut self, schema: &db::Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::postgresql(schema);

        let sql = serializer.serialize(&sql::Statement::create_table(
            table,
            &Capability::POSTGRESQL,
        ));

        self.client
            .execute(&sql, &[])
            .await
            .map_err(toasty_core::Error::driver_operation_failed)?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index));

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
            enum_types: std::collections::HashMap::new(),
        }
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "postgresql", op = %op.name(), "driver exec");

        if let Operation::Transaction(ref t) = op {
            let sql = sql::Serializer::postgresql(&schema.db).serialize_transaction(t);
            self.client.batch_execute(&sql).await.map_err(|e| {
                if let Some(db_err) = e.as_db_error() {
                    match db_err.code().code() {
                        "40001" => toasty_core::Error::serialization_failure(db_err.message()),
                        "25006" => toasty_core::Error::read_only_transaction(db_err.message()),
                        _ => toasty_core::Error::driver_operation_failed(e),
                    }
                } else {
                    toasty_core::Error::driver_operation_failed(e)
                }
            })?;
            return Ok(ExecResponse::count(0));
        }

        let (sql, typed_params, ret_tys): (sql::Statement, Vec<operation::TypedValue>, _) = match op
        {
            Operation::Insert(op) => (op.stmt.into(), op.params, None),
            Operation::QuerySql(query) => {
                assert!(
                    query.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for PostgreSQL"
                );
                (query.stmt.into(), query.params, query.ret)
            }
            op => todo!("op={:#?}", op),
        };

        let width = sql.returning_len();

        let sql_as_str = sql::Serializer::postgresql(&schema.db).serialize(&sql);

        tracing::debug!(db.system = "postgresql", db.statement = %sql_as_str, params = typed_params.len(), "executing SQL");

        let mut param_types = Vec::with_capacity(typed_params.len());
        for tv in &typed_params {
            let pg_type = self.resolve_param_type(&tv.ty).await?;
            param_types.push(pg_type);
        }

        let values: Vec<_> = typed_params
            .into_iter()
            .map(|tv| Value::from(tv.value))
            .collect();
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
            return Ok(ExecResponse::count(count));
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
                Ok(ExecResponse::count(total as _))
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

            Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        }
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        let serializer = sql::Serializer::postgresql(&schema.db);

        // Create PostgreSQL enum types before creating tables.
        // Collect unique enum types across all columns.
        let mut created_enum_types = std::collections::HashSet::new();
        for table in &schema.db.tables {
            for column in &table.columns {
                if let toasty_core::schema::db::Type::Enum(type_enum) = &column.storage_ty
                    && let Some(name) = &type_enum.name
                    && created_enum_types.insert(type_enum.name.clone())
                {
                    // Drop any existing enum type first to avoid "already exists" errors
                    // during test runs or schema resets.
                    let drop_sql = format!("DROP TYPE IF EXISTS \"{}\" CASCADE;", name);
                    self.client
                        .execute(&drop_sql, &[])
                        .await
                        .map_err(toasty_core::Error::driver_operation_failed)?;

                    let sql = serializer.serialize(&sql::Statement::create_enum_type(type_enum));

                    tracing::debug!(enum_type = ?type_enum.name, "creating enum type");
                    self.client
                        .execute(&sql, &[])
                        .await
                        .map_err(toasty_core::Error::driver_operation_failed)?;
                }
            }
        }

        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            self.create_table(&schema.db, table).await?;
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
        name: &str,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        tracing::info!(id = id, name = %name, "applying migration");
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
