mod value;
pub(crate) use value::Value;

use rusqlite::Connection as RusqliteConnection;
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    async_trait,
    driver::{
        operation::{Operation, Transaction},
        Capability, Driver, Response,
    },
    schema::db::{Migration, Schema, SchemaDiff, Table},
    stmt, Result,
};
use toasty_sql::{self as sql, TypedValue};
use url::Url;

#[derive(Debug)]
pub enum Sqlite {
    File(PathBuf),
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
            Ok(Self::File(PathBuf::from(url.path())))
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

    async fn connect(&self) -> toasty_core::Result<Box<dyn toasty_core::Connection>> {
        let connection = match self {
            Sqlite::File(path) => Connection::open(path)?,
            Sqlite::InMemory => Connection::in_memory(),
        };
        Ok(Box::new(connection))
    }

    fn max_connections(&self) -> Option<usize> {
        matches!(self, Self::InMemory).then_some(1)
    }

    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::SQLITE);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| {
                let mut params = Vec::<TypedValue>::new();
                let sql =
                    sql::Serializer::sqlite(stmt.schema()).serialize(stmt.statement(), &mut params);
                assert!(
                    params.is_empty(),
                    "migration statements should not have parameters"
                );
                sql
            })
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

#[derive(Debug)]
pub struct Connection {
    connection: RusqliteConnection,
}

impl Connection {
    pub fn in_memory() -> Self {
        let connection = RusqliteConnection::open_in_memory().unwrap();

        Self { connection }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let connection =
            RusqliteConnection::open(path).map_err(toasty_core::Error::driver_operation_failed)?;
        let sqlite = Self { connection };
        Ok(sqlite)
    }
}

#[toasty_core::async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let (sql, ret_tys): (sql::Statement, _) = match op {
            Operation::QuerySql(op) => {
                assert!(
                    op.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for SQLite"
                );
                (op.stmt.into(), op.ret)
            }
            // Operation::Insert(op) => op.stmt.into(),
            Operation::Transaction(Transaction::Start) => {
                self.connection
                    .execute("BEGIN", [])
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Commit) => {
                self.connection
                    .execute("COMMIT", [])
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Rollback) => {
                self.connection
                    .execute("ROLLBACK", [])
                    .map_err(toasty_core::Error::driver_operation_failed)?;
                return Ok(Response::count(0));
            }
            _ => todo!("op={:#?}", op),
        };

        let mut params: Vec<toasty_sql::TypedValue> = vec![];
        let sql_str = sql::Serializer::sqlite(schema).serialize(&sql, &mut params);

        let mut stmt = self.connection.prepare_cached(&sql_str).unwrap();

        let width = match &sql {
            sql::Statement::Query(stmt) => match &stmt.body {
                stmt::ExprSet::Select(stmt) => {
                    Some(stmt.returning.as_expr_unwrap().as_record_unwrap().len())
                }
                _ => todo!(),
            },
            sql::Statement::Insert(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr_unwrap().as_record_unwrap().len()),
            sql::Statement::Delete(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr_unwrap().as_record_unwrap().len()),
            sql::Statement::Update(stmt) => {
                assert!(stmt.condition.is_none(), "stmt={stmt:#?}");
                stmt.returning
                    .as_ref()
                    .map(|returning| returning.as_expr_unwrap().as_record_unwrap().len())
            }
            _ => None,
        };

        let params = params
            .into_iter()
            .map(|tv| Value::from(tv.value))
            .collect::<Vec<_>>();

        if width.is_none() {
            let count = stmt
                .execute(rusqlite::params_from_iter(params.iter()))
                .map_err(toasty_core::Error::driver_operation_failed)?;

            return Ok(Response::count(count as _));
        }

        let mut rows = stmt
            .query(rusqlite::params_from_iter(params.iter()))
            .unwrap();

        let mut ret = vec![];

        let ret_tys = &ret_tys.as_ref().unwrap();

        loop {
            match rows.next() {
                Ok(Some(row)) => {
                    let mut items = vec![];

                    let width = width.unwrap();

                    for index in 0..width {
                        items.push(Value::from_sql(row, index, &ret_tys[index]).into_inner());
                    }

                    ret.push(stmt::ValueRecord::from_vec(items).into());
                }
                Ok(None) => break,
                Err(err) => {
                    return Err(toasty_core::Error::driver_operation_failed(err));
                }
            }
        }

        Ok(Response::value_stream(stmt::ValueStream::from_vec(ret)))
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
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
                let id: u64 = row.get(0)?;
                Ok(toasty_core::schema::db::AppliedMigration::new(id))
            })
            .map_err(toasty_core::Error::driver_operation_failed)?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(toasty_core::Error::driver_operation_failed)
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: String,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
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
            rusqlite::params![id, name],
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
    fn create_table(&mut self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::sqlite(schema);

        let mut params: Vec<toasty_sql::TypedValue> = vec![];
        let stmt = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::SQLITE),
            &mut params,
        );
        assert!(params.is_empty());

        self.connection
            .execute(&stmt, [])
            .map_err(toasty_core::Error::driver_operation_failed)?;

        // Create any indices
        for index in &table.indices {
            // The PK has already been created by the table statement
            if index.primary_key {
                continue;
            }

            let stmt = serializer.serialize(&sql::Statement::create_index(index), &mut params);
            assert!(params.is_empty());

            self.connection
                .execute(&stmt, [])
                .map_err(toasty_core::Error::driver_operation_failed)?;
        }
        Ok(())
    }
}
