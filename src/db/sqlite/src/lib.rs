use toasty_core::{
    driver::{operation::Operation, Capability, Driver, Response},
    schema::db::{Schema, Table},
    stmt,
};

use anyhow::Result;
use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct Sqlite {
    connection: Mutex<Connection>,
}

impl Sqlite {
    pub fn in_memory() -> Sqlite {
        let connection = Connection::open_in_memory().unwrap();

        Sqlite {
            connection: Mutex::new(connection),
        }
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Sqlite> {
        let connection = Connection::open(path)?;
        let sqlite = Sqlite {
            connection: Mutex::new(connection),
        };
        Ok(sqlite)
    }
}

#[toasty_core::async_trait]
impl Driver for Sqlite {
    fn capability(&self) -> &Capability {
        &Capability::Sql
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        use Operation::*;

        let connection = self.connection.lock().unwrap();

        let mut sql = match op {
            QuerySql(op) => op.stmt,
            Insert(op) => op,
            _ => todo!(),
        };

        // SQL doesn't handle pre-condition. This should be moved into toasty's planner.
        let pre_condition = match &mut sql {
            stmt::Statement::Update(update) => {
                if let Some(condition) = update.condition.take() {
                    update.filter = Some(match update.filter.take() {
                        Some(filter) => stmt::Expr::and(filter, condition),
                        None => condition,
                    });

                    assert!(update.returning.is_none());

                    update.returning = Some(stmt::Returning::Expr(stmt::Expr::record([true])));

                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        let mut params = vec![];
        let sql_str = stmt::sql::Serializer::new(schema).serialize_stmt(&sql, &mut params);

        let mut stmt = connection.prepare(&sql_str).unwrap();

        let width = match &sql {
            stmt::Statement::Query(stmt) => match &*stmt.body {
                stmt::ExprSet::Select(stmt) => Some(stmt.returning.as_expr().as_record().len()),
                _ => todo!(),
            },
            stmt::Statement::Insert(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr().as_record().len()),
            stmt::Statement::Delete(stmt) => stmt
                .returning
                .as_ref()
                .map(|returning| returning.as_expr().as_record().len()),
            stmt::Statement::Update(stmt) => {
                assert!(stmt.condition.is_none(), "stmt={stmt:#?}");
                stmt.returning
                    .as_ref()
                    .map(|returning| returning.as_expr().as_record().len())
            }
        };

        if width.is_none() {
            let count = stmt.execute(rusqlite::params_from_iter(
                params.iter().map(value_from_param),
            ))?;

            return Ok(Response::from_count(count));
        }

        let mut rows = stmt
            .query(rusqlite::params_from_iter(
                params.iter().map(value_from_param),
            ))
            .unwrap();

        let mut ret = vec![];

        loop {
            match rows.next() {
                Ok(Some(row)) => {
                    let mut items = vec![];

                    let width = width.unwrap();

                    for index in 0..width {
                        items.push(load(row, index));
                    }

                    ret.push(stmt::ValueRecord::from_vec(items).into());
                }
                Ok(None) => break,
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        // Some special handling
        if sql.is_update() && pre_condition {
            if ret.is_empty() {
                // Just assume the precondition failed here... we will
                // need to make this transactional later.
                anyhow::bail!("pre condition failed");
            } else {
                return Ok(Response::from_count(ret.len()));
            }
        }

        Ok(Response::from_value_stream(stmt::ValueStream::from_vec(
            ret,
        )))
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
        let connection = self.connection.lock().unwrap();

        let mut params = vec![];
        let stmt = stmt::sql::Statement::create_table(table).serialize(schema, &mut params);
        assert!(params.is_empty());

        connection.execute(&stmt, [])?;

        // Create any indices
        for index in &table.indices {
            // The PK has already been created by the table statement
            if index.primary_key {
                continue;
            }

            let stmt = stmt::sql::Statement::create_index(index).serialize(schema, &mut params);
            assert!(params.is_empty());

            connection.execute(&stmt, [])?;
        }
        Ok(())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum V {
    Bool(bool),
    Null,
    String(String),
    I64(i64),
    Id(usize, String),
}

fn value_from_param(value: &stmt::Value) -> rusqlite::types::ToSqlOutput<'_> {
    use rusqlite::types::{ToSqlOutput, Value, ValueRef};
    use stmt::Value::*;

    match value {
        Bool(true) => ToSqlOutput::Owned(Value::Integer(1)),
        Bool(false) => ToSqlOutput::Owned(Value::Integer(0)),
        Id(v) => ToSqlOutput::Owned(v.to_string().into()),
        I64(v) => ToSqlOutput::Owned(Value::Integer(*v)),
        String(v) => ToSqlOutput::Borrowed(ValueRef::Text(v.as_bytes())),
        Null => ToSqlOutput::Owned(Value::Null),
        Enum(value_enum) => {
            let v = match &value_enum.fields[..] {
                [] => V::Null,
                [stmt::Value::Bool(v)] => V::Bool(*v),
                [stmt::Value::String(v)] => V::String(v.to_string()),
                [stmt::Value::I64(v)] => V::I64(*v),
                [stmt::Value::Id(id)] => V::Id(id.model_id().0, id.to_string()),
                _ => todo!("val={:#?}", value_enum.fields),
            };

            ToSqlOutput::Owned(
                format!(
                    "{}#{}",
                    value_enum.variant,
                    serde_json::to_string(&v).unwrap()
                )
                .into(),
            )
        }
        _ => todo!("value = {:#?}", value),
    }
}

fn load(row: &rusqlite::Row, index: usize) -> stmt::Value {
    use rusqlite::types::Value as SqlValue;

    let value: Option<SqlValue> = row.get(index).unwrap();

    match value {
        Some(SqlValue::Null) => stmt::Value::Null,
        Some(SqlValue::Integer(value)) => stmt::Value::I64(value),
        Some(SqlValue::Text(value)) => stmt::Value::String(value),
        None => stmt::Value::Null,
        _ => todo!("value={value:#?}"),
    }
}
