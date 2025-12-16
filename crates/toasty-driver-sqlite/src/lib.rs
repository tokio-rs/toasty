mod value;
pub(crate) use value::Value;

use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use toasty_core::{
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
pub struct Sqlite {
    connection: Mutex<Connection>,
}

impl Sqlite {
    pub fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "sqlite" {
            return Err(anyhow::anyhow!(
                "connection URL does not have a `sqlite` scheme; url={url}"
            ));
        }

        if url.path() == ":memory:" {
            Ok(Self::in_memory())
        } else {
            Self::open(url.path())
        }
    }

    pub fn in_memory() -> Self {
        let connection = Connection::open_in_memory().unwrap();

        Self {
            connection: Mutex::new(connection),
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let connection = Connection::open(path)?;
        let sqlite = Self {
            connection: Mutex::new(connection),
        };
        Ok(sqlite)
    }
}

#[toasty_core::async_trait]
impl Driver for Sqlite {
    fn capability(&self) -> &'static Capability {
        &Capability::SQLITE
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let connection = self.connection.lock().unwrap();

        let (sql, ret_tys): (sql::Statement, _) = match op {
            Operation::QuerySql(op) => (op.stmt.into(), op.ret),
            // Operation::Insert(op) => op.stmt.into(),
            Operation::Transaction(Transaction::Start) => {
                connection.execute("BEGIN", [])?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Commit) => {
                connection.execute("COMMIT", [])?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Rollback) => {
                connection.execute("ROLLBACK", [])?;
                return Ok(Response::count(0));
            }
            _ => todo!("op={:#?}", op),
        };

        let mut params = vec![];
        let sql_str = sql::Serializer::sqlite(schema).serialize(&sql, &mut params);

        let mut stmt = connection.prepare(&sql_str).unwrap();

        let width = match &sql {
            sql::Statement::Query(stmt) => match &stmt.body {
                stmt::ExprSet::Select(stmt) => {
                    Some(stmt.returning.as_expr_unwrap().as_record().len())
                }
                _ => todo!(),
            },
            sql::Statement::Insert(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr_unwrap().as_record().len()),
            sql::Statement::Delete(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr_unwrap().as_record().len()),
            sql::Statement::Update(stmt) => {
                assert!(stmt.condition.is_none(), "stmt={stmt:#?}");
                stmt.returning
                    .as_ref()
                    .map(|returning| returning.as_expr_unwrap().as_record().len())
            }
            _ => None,
        };

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();

        if width.is_none() {
            let count = stmt.execute(rusqlite::params_from_iter(params.iter()))?;

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
                    return Err(err.into());
                }
            }
        }

        Ok(Response::value_stream(stmt::ValueStream::from_vec(ret)))
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in &schema.tables {
            self.create_table(schema, table)?;
        }

        Ok(())
    }
}

impl Sqlite {
    fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::sqlite(schema);

        let connection = self.connection.lock().unwrap();

        let mut params = vec![];
        let stmt = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::SQLITE),
            &mut params,
        );
        assert!(params.is_empty());

        connection.execute(&stmt, [])?;

        // Create any indices
        for index in &table.indices {
            // The PK has already been created by the table statement
            if index.primary_key {
                continue;
            }

            let stmt = serializer.serialize(&sql::Statement::create_index(index), &mut params);
            assert!(params.is_empty());

            connection.execute(&stmt, [])?;
        }
        Ok(())
    }
}
