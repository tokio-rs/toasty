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
use tokio_postgres::{Client, Config};

#[derive(Debug)]
pub struct PostgreSQL {
    /// The PostgreSQL client.
    client: Client,
}

impl PostgreSQL {
    /// Connects to a PostgreSQL database using a connection string.
    ///
    /// See [`postgres::Client::connect`] for more information.
    pub async fn connect<T>(params: &str, tls: T) -> Result<Self>
    where
        T: MakeTlsConnect<Socket> + 'static,
        T::Stream: Send,
    {
        let (client, connection) = tokio_postgres::connect(params, tls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(Self { client })
    }

    /// Connects to a PostgreSQL database using a [`postgres::Config`].
    ///
    /// See [`postgres::Client::configure`] for more information.
    pub async fn connect_using_config<T>(config: Config, tls: T) -> Result<Self>
    where
        T: MakeTlsConnect<Socket> + 'static,
        T::Stream: Send,
    {
        let (client, connection) = config.connect(tls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        Ok(Self { client })
    }

    /// Creates a table.
    pub async fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let mut params = Vec::new();
        let sql = stmt::sql::Statement::create_table(table)
            .serialize(schema, &mut params)
            .into_inner();

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

            let sql = stmt::sql::Statement::create_index(index)
                .serialize(schema, &mut params)
                .into_inner();
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
        let mut params = Vec::new();

        let sql = if if_exists {
            stmt::sql::Statement::drop_table_if_exists(table)
                .serialize(schema, &mut params)
                .into_inner()
        } else {
            stmt::sql::Statement::drop_table(table)
                .serialize(schema, &mut params)
                .into_inner()
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
        &Capability::Sql
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let sql = match op {
            Operation::Insert(stmt) => stmt,
            Operation::QuerySql(query) => query.stmt,
            op => todo!("op={:#?}", op),
        };

        let width = sql.returning_len();

        let mut params = Vec::new();
        let sql_as_str = stmt::sql::Serializer::new(schema)
            .serialize_stmt(&sql, &mut params)
            .into_numbered_args()
            .into_inner();

        let stmt = self.client.prepare(&sql_as_str).await?;
        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();

        let args = params
            .iter()
            .map(|param| param as &(dyn ToSql + Sync))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        if width.is_none() {
            let count = self.client.execute(&stmt, &args).await? as usize;
            return Ok(Response::from_count(count));
        }

        let results = self
            .client
            .query(&stmt, &args)
            .await?
            .into_iter()
            .map(|row| {
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
    if column.type_() == &Type::TEXT {
        row.get::<usize, Option<String>>(index)
            .map(stmt::Value::String)
            .unwrap_or(stmt::Value::Null)
    } else if column.type_() == &Type::BOOL {
        row.get::<usize, Option<bool>>(index)
            .map(stmt::Value::Bool)
            .unwrap_or(stmt::Value::Null)
    } else if column.type_() == &Type::INT4 {
        row.get::<usize, Option<i64>>(index)
            .map(stmt::Value::I64)
            .unwrap_or(stmt::Value::Null)
    } else {
        todo!(
            "implement PostgreSQL to toasty conversion for `{:#?}`",
            column.type_()
        );
    }
}
