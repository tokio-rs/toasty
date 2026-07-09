#![warn(missing_docs)]

//! Toasty driver for [SQLite](https://www.sqlite.org/) using
//! [`rusqlite`](https://docs.rs/rusqlite).
//!
//! Supports both file-backed and in-memory databases.
//!
//! # Examples
//!
//! ```
//! use toasty_driver_sqlite::Sqlite;
//!
//! // In-memory database
//! let driver = Sqlite::in_memory();
//!
//! // File-backed database
//! let driver = Sqlite::open("path/to/db.sqlite3");
//! ```

mod value;
pub(crate) use value::Value;

use async_trait::async_trait;
use rusqlite::Connection as RusqliteConnection;
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    Result,
    driver::{
        Capability, ConnectContext, Driver, ExecResponse, QueryLogConfig,
        log::QueryLog,
        operation::{IsolationLevel, Operation, RawSqlRet, Transaction, TypedValue},
    },
    schema::{
        db::{self, Migration, Table},
        diff,
    },
    stmt,
};
use toasty_sql::{self as sql};
use url::Url;

enum SqlReturn {
    Count,
    Infer,
    Types(Vec<stmt::Type>),
}

/// A SQLite [`Driver`] that opens connections to a file or in-memory database.
///
/// # Examples
///
/// ```
/// use toasty_driver_sqlite::Sqlite;
///
/// let driver = Sqlite::in_memory();
/// ```
#[derive(Debug)]
pub enum Sqlite {
    /// A database stored at a filesystem path.
    File(PathBuf),
    /// An ephemeral in-memory database.
    InMemory,
}

impl Sqlite {
    /// Create a new SQLite driver with an arbitrary connection URL
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = Url::parse(&url_str).map_err(toasty_core::Error::driver_operation_failed)?;

        if url.scheme() != "sqlite" {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection URL does not have a `sqlite` scheme; url={}",
                url_str
            )));
        }

        if url.path() == ":memory:" {
            Ok(Self::InMemory)
        } else {
            Ok(Self::File(PathBuf::from(
                percent_encoding::percent_decode(url.path().as_bytes())
                    .decode_utf8_lossy()
                    .to_string()
                    .as_str(),
            )))
        }
    }

    /// Create an in-memory SQLite database
    pub fn in_memory() -> Self {
        Self::InMemory
    }

    /// Open a SQLite database at the specified file path
    pub fn open<P: AsRef<Path>>(path: P) -> Self {
        Self::File(path.as_ref().to_path_buf())
    }
}

#[async_trait]
impl Driver for Sqlite {
    fn url(&self) -> Cow<'_, str> {
        match self {
            Sqlite::InMemory => Cow::Borrowed("sqlite::memory:"),
            Sqlite::File(path) => Cow::Owned(format!("sqlite:{}", path.display())),
        }
    }

    fn capability(&self) -> &'static Capability {
        &Capability::SQLITE
    }

    async fn connect(
        &self,
        cx: &ConnectContext,
    ) -> toasty_core::Result<Box<dyn toasty_core::Connection>> {
        let mut connection = match self {
            Sqlite::File(path) => Connection::open(path)?,
            Sqlite::InMemory => Connection::in_memory(),
        };
        connection.query_log = cx.query_log;
        Ok(Box::new(connection))
    }

    fn max_connections(&self) -> Option<usize> {
        matches!(self, Self::InMemory).then_some(1)
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::SQLITE);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::sqlite(stmt.schema()).serialize(stmt.statement()))
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        match self {
            Sqlite::File(path) => {
                // Delete the file and recreate it
                if path.exists() {
                    std::fs::remove_file(path)
                        .map_err(toasty_core::Error::driver_operation_failed)?;
                }
            }
            Sqlite::InMemory => {
                // Nothing to do — each connect() creates a fresh in-memory database
            }
        }

        Ok(())
    }
}

/// An open connection to a SQLite database.
#[derive(Debug)]
pub struct Connection {
    connection: RusqliteConnection,
    query_log: QueryLogConfig,
}

impl Connection {
    /// Open an in-memory SQLite connection.
    pub fn in_memory() -> Self {
        let connection = RusqliteConnection::open_in_memory().unwrap();

        Self {
            connection,
            query_log: QueryLogConfig::default(),
        }
    }

    /// Open a SQLite connection to a file at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let connection =
            RusqliteConnection::open(path).map_err(toasty_core::Error::driver_operation_failed)?;
        let sqlite = Self {
            connection,
            query_log: QueryLogConfig::default(),
        };
        Ok(sqlite)
    }

    fn exec_sql(
        &mut self,
        sql_str: &str,
        typed_params: Vec<TypedValue>,
        ret: SqlReturn,
    ) -> Result<ExecResponse> {
        let mut log = QueryLog::sql(
            &self.query_log,
            "sqlite",
            sql_str,
            typed_params.iter().map(|tv| &tv.value),
        );
        let result = self.exec_sql_inner(sql_str, typed_params, ret, &mut log);
        log.finish(&result);
        result
    }

    fn exec_sql_inner(
        &mut self,
        sql_str: &str,
        typed_params: Vec<TypedValue>,
        ret: SqlReturn,
        log: &mut QueryLog<'_>,
    ) -> Result<ExecResponse> {
        let mut stmt = self
            .connection
            .prepare_cached(sql_str)
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let params = typed_params
            .into_iter()
            .map(|tv| Value::from(tv.value))
            .collect::<Vec<_>>();

        if matches!(ret, SqlReturn::Count) {
            let count = stmt
                .execute(rusqlite::params_from_iter(params.iter()))
                .map_err(toasty_core::Error::driver_operation_failed)?;

            return Ok(ExecResponse::count(count as _));
        }

        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.iter()))
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let mut values = vec![];
        let column_count = rows.as_ref().map(|stmt| stmt.column_count()).unwrap_or(0);

        loop {
            match rows.next() {
                Ok(Some(row)) => {
                    let items = match &ret {
                        SqlReturn::Count => unreachable!(),
                        SqlReturn::Infer => (0..column_count)
                            .map(|index| Value::from_sql_infer(row, index).into_inner())
                            .collect(),
                        SqlReturn::Types(ret_tys) => ret_tys
                            .iter()
                            .enumerate()
                            .map(|(index, ret_ty)| Value::from_sql(row, index, ret_ty).into_inner())
                            .collect(),
                    };

                    values.push(stmt::ValueRecord::from_vec(items).into());
                }
                Ok(None) => break,
                Err(err) => {
                    return Err(toasty_core::Error::driver_operation_failed(err));
                }
            }
        }

        log.rows(values.len() as u64);
        Ok(ExecResponse::value_stream(stmt::ValueStream::from_vec(
            values,
        )))
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<db::Schema>, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "sqlite", op = %op.name(), "driver exec");

        let (sql, typed_params, ret_tys) = match op {
            Operation::QuerySql(op) => {
                assert!(
                    op.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for SQLite"
                );
                (sql::Statement::from(op.stmt), op.params, op.ret)
            }
            Operation::RawSql(op) => {
                let ret = match op.ret {
                    RawSqlRet::None => SqlReturn::Count,
                    RawSqlRet::Infer => SqlReturn::Infer,
                    RawSqlRet::Types(types) => SqlReturn::Types(types),
                };
                return self.exec_sql(&op.sql, op.params, ret);
            }
            // Operation::Insert(op) => op.stmt.into(),
            Operation::Transaction(mut op) => {
                if let Transaction::Start { isolation, .. } = &mut op {
                    if !matches!(isolation, Some(IsolationLevel::Serializable) | None) {
                        return Err(toasty_core::Error::unsupported_feature(
                            "SQLite only supports Serializable isolation",
                        ));
                    }
                    *isolation = None;
                }
                let sql = sql::Serializer::sqlite(schema).serialize_transaction(&op);
                self.connection
                    .execute(&sql, [])
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(ExecResponse::count(0));
            }
            _ => todo!("op={:#?}", op),
        };

        let ret = match &sql {
            sql::Statement::Query(stmt) => match &stmt.body {
                stmt::ExprSet::Select(_) => SqlReturn::Types(ret_tys.unwrap()),
                _ => todo!(),
            },
            sql::Statement::Insert(stmt) => stmt
                .returning
                .as_ref()
                .map(|_| SqlReturn::Types(ret_tys.unwrap()))
                .unwrap_or(SqlReturn::Count),
            sql::Statement::Delete(stmt) => stmt
                .returning
                .as_ref()
                .map(|_| SqlReturn::Types(ret_tys.unwrap()))
                .unwrap_or(SqlReturn::Count),
            sql::Statement::Update(stmt) => {
                assert!(stmt.condition.is_none(), "stmt={stmt:#?}");
                stmt.returning
                    .as_ref()
                    .map(|_| SqlReturn::Types(ret_tys.unwrap()))
                    .unwrap_or(SqlReturn::Count)
            }
            _ => SqlReturn::Count,
        };

        let sql_str = sql::Serializer::sqlite(schema).serialize(&sql);
        self.exec_sql(&sql_str, typed_params, ret)
    }

    async fn push_schema(&mut self, schema: &db::Schema) -> Result<()> {
        for table in &schema.tables {
            tracing::debug!(table = %table.name, "creating table");
            self.create_table(schema, table)?;
        }

        Ok(())
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        // Ensure the migrations table exists
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )",
                [],
            )
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Query all applied migrations
        let mut stmt = self
            .connection
            .prepare("SELECT id FROM __toasty_migrations ORDER BY applied_at")
            .map_err(toasty_core::Error::driver_operation_failed)?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                Ok(toasty_core::schema::db::AppliedMigration::new(id as u64))
            })
            .map_err(toasty_core::Error::driver_operation_failed)?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(toasty_core::Error::driver_operation_failed)
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        tracing::info!(id = id, name = %name, "applying migration");
        // Ensure the migrations table exists
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )",
                [],
            )
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Start transaction
        self.connection
            .execute("BEGIN", [])
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Execute each migration statement
        for statement in migration.statements() {
            if let Err(e) = self
                .connection
                .execute(statement, [])
                .map_err(toasty_core::Error::driver_operation_failed)
            {
                self.connection
                    .execute("ROLLBACK", [])
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Err(e);
            }
        }

        // Record the migration
        if let Err(e) = self.connection.execute(
            "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES (?1, ?2, datetime('now'))",
            rusqlite::params![id as i64, name],
        ).map_err(toasty_core::Error::driver_operation_failed) {
            self.connection.execute("ROLLBACK", []).map_err(toasty_core::Error::driver_operation_failed)?;
            return Err(e);
        }

        // Commit transaction
        self.connection
            .execute("COMMIT", [])
            .map_err(toasty_core::Error::driver_operation_failed)?;
        Ok(())
    }
}

impl Connection {
    fn create_table(&mut self, schema: &db::Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::sqlite(schema);

        let stmt = serializer.serialize(&sql::Statement::create_table(table, &Capability::SQLITE));

        self.connection
            .execute(&stmt, [])
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Create any indices
        for index in &table.indices {
            // The PK has already been created by the table statement
            if index.primary_key {
                continue;
            }

            let stmt = serializer.serialize(&sql::Statement::create_index(index));

            self.connection
                .execute(&stmt, [])
                .map_err(toasty_core::Error::driver_operation_failed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Sqlite;
    use std::path::PathBuf;

    /// The file path `Sqlite::new` resolves out of a `sqlite:` URL.
    fn file_path(url: &str) -> PathBuf {
        match Sqlite::new(url).unwrap() {
            Sqlite::File(path) => path,
            Sqlite::InMemory => panic!("expected a file-backed database for {url}"),
        }
    }

    #[test]
    fn new_decodes_percent_encoded_path() {
        // `url::Url` stores the path percent-encoded: a space becomes `%20` and
        // non-ASCII bytes become `%XX` sequences. The driver must decode it back
        // before opening the file, otherwise it opens one whose name literally
        // contains `%20`.
        assert_eq!(
            file_path("sqlite:/tmp/my db.sqlite"),
            PathBuf::from("/tmp/my db.sqlite")
        );
        assert_eq!(
            file_path("sqlite:///tmp/my%20db.sqlite"),
            PathBuf::from("/tmp/my db.sqlite")
        );
        assert_eq!(
            file_path("sqlite:/tmp/d%C3%A9j%C3%A0.db"),
            PathBuf::from("/tmp/déjà.db")
        );
        // Percent-decoding, not form-decoding: a literal `+` must stay a `+`.
        assert_eq!(
            file_path("sqlite:/tmp/a+b.db"),
            PathBuf::from("/tmp/a+b.db")
        );
    }

    #[test]
    fn new_memory_url_stays_in_memory() {
        assert!(matches!(
            Sqlite::new("sqlite::memory:").unwrap(),
            Sqlite::InMemory
        ));
    }
}
