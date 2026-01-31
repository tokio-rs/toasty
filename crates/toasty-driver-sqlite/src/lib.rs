mod value;
pub(crate) use value::Value;

use rusqlite::Connection as RusqliteConnection;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    async_trait,
    driver::{
        operation::{Operation, Transaction},
        Capability, Driver, Response,
    },
    schema::db::{Schema, Table},
    stmt, Result,
};
use toasty_sql as sql;
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

    async fn reset_db(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.create_table(schema, table)?;
        }

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
