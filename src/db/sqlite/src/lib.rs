use toasty_core::{
    driver::{operation::Operation, Capability, Driver},
    schema, stmt, Schema,
};

use anyhow::Result;
use rusqlite::Connection;
use std::{path::Path, sync::Mutex};

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

    async fn exec<'stmt>(
        &self,
        schema: &Schema,
        op: Operation<'stmt>,
    ) -> Result<stmt::ValueStream<'stmt>> {
        use Operation::*;

        let connection = self.connection.lock().unwrap();

        let sql;
        let ty;

        match &op {
            QuerySql(op) => {
                sql = &op.stmt;
                ty = op.ty.as_ref();
            }
            Insert(op) => {
                sql = op;
                ty = None;
            }
            _ => todo!(),
        }

        let mut params = vec![];
        let sql_str = stmt::sql::Serializer::new(schema).serialize(sql, &mut params);

        let mut stmt = connection.prepare(&sql_str).unwrap();

        if ty.is_none() {
            let exec = !matches!(sql, stmt::Statement::Update(u) if u.pre_condition.is_some());

            if exec {
                stmt.execute(rusqlite::params_from_iter(
                    params.iter().map(value_from_param),
                ))
                .unwrap();

                return Ok(stmt::ValueStream::new());
            }
        }

        let mut rows = stmt
            .query(rusqlite::params_from_iter(
                params.iter().map(value_from_param),
            ))
            .unwrap();

        let ty = match ty {
            Some(ty) => ty,
            None => &stmt::Type::Bool,
        };

        let mut ret = vec![];

        loop {
            match rows.next() {
                Ok(Some(row)) => {
                    if let stmt::Type::Record(tys) = ty {
                        let mut items = vec![];

                        for (index, ty) in tys.iter().enumerate() {
                            items.push(load(row, index, ty));
                        }

                        ret.push(stmt::Record::from_vec(items).into());
                    } else if let stmt::Type::Bool = ty {
                        ret.push(stmt::Record::from_vec(vec![]).into());
                    } else {
                        todo!()
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        // Some special handling
        if let sql::Statement::Update(update) = sql {
            if update.pre_condition.is_some() && ret.is_empty() {
                // Just assume the precondition failed here... we will
                // need to make this transactional later.
                anyhow::bail!("pre condition failed");
            } else if update.returning.is_none() {
                return Ok(stmt::ValueStream::new());
            }
        }

        Ok(stmt::ValueStream::from_vec(ret))
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        for table in schema.tables() {
            self.create_table(schema, table)?;
        }

        Ok(())
    }
}

impl Sqlite {
    fn create_table(&self, schema: &Schema, table: &schema::Table) -> Result<()> {
        let connection = self.connection.lock().unwrap();

        let mut params = vec![];
        let stmt = sql::Statement::create_table(table).to_sql_string(schema, &mut params);
        assert!(params.is_empty());

        connection.execute(&stmt, [])?;

        // Create any indices
        for index in &table.indices {
            // The PK has already been created by the table statement
            if index.primary_key {
                continue;
            }

            let stmt = sql::Statement::create_index(index).to_sql_string(schema, &mut params);
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

fn value_from_param<'a>(value: &'a stmt::Value<'a>) -> rusqlite::types::ToSqlOutput<'a> {
    use rusqlite::types::{ToSqlOutput, Value, ValueRef};
    use stmt::Value::*;

    match value {
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

fn load<'stmt>(row: &rusqlite::Row, index: usize, ty: &stmt::Type) -> stmt::Value<'stmt> {
    use std::borrow::Cow;

    match ty {
        stmt::Type::Id(mid) => {
            let s: Option<String> = row.get(index).unwrap();
            match s {
                Some(s) => stmt::Id::from_string(*mid, s).into(),
                None => stmt::Value::Null,
            }
        }
        stmt::Type::String => {
            let s: Option<String> = row.get(index).unwrap();
            match s {
                Some(s) => stmt::Value::String(Cow::Owned(s)),
                None => stmt::Value::Null,
            }
        }
        stmt::Type::I64 => {
            let s: Option<i64> = row.get(index).unwrap();
            match s {
                Some(s) => stmt::Value::I64(s),
                None => stmt::Value::Null,
            }
        }
        stmt::Type::Enum(..) => {
            let s: Option<String> = row.get(index).unwrap();

            match s {
                Some(s) => {
                    let (variant, rest) = s.split_once("#").unwrap();
                    let variant: usize = variant.parse().unwrap();
                    let v: V = serde_json::from_str(rest).unwrap();
                    let value = match v {
                        V::Bool(v) => stmt::Value::Bool(v),
                        V::Null => stmt::Value::Null,
                        V::String(v) => stmt::Value::String(v.into()),
                        V::Id(model, v) => {
                            stmt::Value::Id(stmt::Id::from_string(schema::ModelId(model), v))
                        }
                        V::I64(v) => stmt::Value::I64(v),
                    };

                    if value.is_null() {
                        stmt::ValueEnum {
                            variant,
                            fields: stmt::Record::from_vec(vec![]),
                        }
                        .into()
                    } else {
                        stmt::ValueEnum {
                            variant,
                            fields: stmt::Record::from_vec(vec![value]),
                        }
                        .into()
                    }
                }
                None => stmt::Value::Null,
            }
        }
        ty => todo!("column.ty = {:#?}", ty),
    }
}
