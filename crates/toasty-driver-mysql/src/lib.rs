#![allow(clippy::needless_range_loop)]

mod value;
pub(crate) use value::Value;

use mysql_async::{
    consts::ColumnType,
    prelude::{Queryable, ToValue},
    Pool,
};
use std::sync::Arc;
use toasty_core::{
    driver::{operation::Transaction, Capability, Operation, Response},
    schema::db::{Schema, Table},
    stmt::{self, ValueRecord},
    Driver, Result,
};
use toasty_sql as sql;
use url::Url;

#[derive(Debug)]
pub struct MySQL {
    pool: Pool,
}

impl MySQL {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        if url.scheme() != "mysql" {
            return Err(anyhow::anyhow!(
                "connection url does not have a `mysql` scheme; url={}",
                url
            ));
        }

        url.host_str()
            .ok_or_else(|| anyhow::anyhow!("missing host in connection URL; url={}", url))?;

        if url.path().is_empty() {
            return Err(anyhow::anyhow!(
                "no database specified - missing path in connection URL; url={}",
                url
            ));
        }

        let opts = mysql_async::Opts::from_url(url.as_ref())?;
        let opts = mysql_async::OptsBuilder::from_opts(opts).client_found_rows(true);

        let pool = Pool::new(opts);
        Ok(Self { pool })
    }

    pub async fn create_table(&self, schema: &Schema, table: &Table) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);

        let mut params = Vec::new();

        let sql = serializer.serialize(
            &sql::Statement::create_table(table, &Capability::MYSQL),
            &mut params,
        );

        assert!(
            params.is_empty(),
            "creating a table shouldn't involve any parameters"
        );

        let mut conn = self.pool.get_conn().await?;
        conn.exec_drop(&sql, ()).await?;

        for index in &table.indices {
            if index.primary_key {
                continue;
            }

            let sql = serializer.serialize(&sql::Statement::create_index(index), &mut params);

            assert!(
                params.is_empty(),
                "creating an index shouldn't involve any parameters"
            );

            conn.exec_drop(&sql, ()).await?;
        }

        Ok(())
    }

    /// Drops a table.
    pub async fn drop_table(&self, schema: &Schema, table: &Table, if_exists: bool) -> Result<()> {
        let serializer = sql::Serializer::mysql(schema);
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

        let mut conn = self.pool.get_conn().await?;

        conn.exec_drop(&sql, ()).await?;
        Ok(())
    }
}
impl From<Pool> for MySQL {
    fn from(pool: Pool) -> Self {
        Self { pool }
    }
}

#[toasty_core::async_trait]
impl Driver for MySQL {
    fn capability(&self) -> &Capability {
        &Capability::MYSQL
    }

    async fn register_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        let mut conn = self.pool.get_conn().await?;

        let (sql, ret): (sql::Statement, _) = match op {
            // Operation::Insert(stmt) => stmt.into(),
            Operation::QuerySql(op) => (op.stmt.into(), op.ret),
            Operation::Transaction(Transaction::Start) => {
                conn.query_drop("START TRANSACTION").await?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Commit) => {
                conn.query_drop("COMMIT").await?;
                return Ok(Response::count(0));
            }
            Operation::Transaction(Transaction::Rollback) => {
                conn.query_drop("ROLLBACK").await?;
                return Ok(Response::count(0));
            }
            op => todo!("op={:#?}", op),
        };

        let mut params = Vec::new();

        let sql_as_str = sql::Serializer::mysql(schema).serialize(&sql, &mut params);

        let params = params.into_iter().map(Value::from).collect::<Vec<_>>();
        let args = params
            .iter()
            .map(|param| param.to_value())
            .collect::<Vec<_>>();

        if ret.is_none() {
            let count = conn
                .exec_iter(&sql_as_str, mysql_async::Params::Positional(args))
                .await?
                .affected_rows();

            return Ok(Response::count(count));
        }

        let rows: Vec<mysql_async::Row> = conn.exec(&sql_as_str, &args).await?;

        if let Some(returning) = ret {
            let results = rows.into_iter().map(move |mut row| {
                assert_eq!(
                    row.len(),
                    returning.len(),
                    "row={row:#?}; returning={returning:#?}"
                );

                let mut results = Vec::new();
                for i in 0..row.len() {
                    let column = &row.columns()[i];
                    results.push(mysql_to_toasty(i, &mut row, column, &returning[i]));
                }

                Ok(ValueRecord::from_vec(results))
            });

            Ok(Response::value_stream(stmt::ValueStream::from_iter(
                results,
            )))
        } else {
            let [row] = &rows[..] else { todo!() };
            let total = row.get::<i64, usize>(0).unwrap();
            let condition_matched = row.get::<i64, usize>(1).unwrap();

            if total == condition_matched {
                Ok(Response::count(total as _))
            } else {
                anyhow::bail!("update condition did not match");
            }
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

fn mysql_to_toasty(
    i: usize,
    row: &mut mysql_async::Row,
    column: &mysql_async::Column,
    ty: &stmt::Type,
) -> stmt::Value {
    use ColumnType::*;

    match column.column_type() {
        MYSQL_TYPE_NULL => stmt::Value::Null,

        MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING | MYSQL_TYPE_STRING | MYSQL_TYPE_BLOB => {
            match ty {
                stmt::Type::String => extract_or_null(row, i, stmt::Value::String),
                stmt::Type::Uuid => extract_or_null(row, i, stmt::Value::Uuid),
                stmt::Type::Bytes => extract_or_null(row, i, stmt::Value::Bytes),
                _ => todo!("ty={ty:#?}"),
            }
        }

        MYSQL_TYPE_TINY | MYSQL_TYPE_SHORT | MYSQL_TYPE_INT24 | MYSQL_TYPE_LONG
        | MYSQL_TYPE_LONGLONG => match ty {
            stmt::Type::Bool => extract_or_null(row, i, stmt::Value::Bool),
            stmt::Type::I8 => extract_or_null(row, i, stmt::Value::I8),
            stmt::Type::I16 => extract_or_null(row, i, stmt::Value::I16),
            stmt::Type::I32 => extract_or_null(row, i, stmt::Value::I32),
            stmt::Type::I64 => extract_or_null(row, i, stmt::Value::I64),
            stmt::Type::U8 => extract_or_null(row, i, stmt::Value::U8),
            stmt::Type::U16 => extract_or_null(row, i, stmt::Value::U16),
            stmt::Type::U32 => extract_or_null(row, i, stmt::Value::U32),
            stmt::Type::U64 => extract_or_null(row, i, stmt::Value::U64),
            _ => todo!("ty={ty:#?}"),
        },

        #[cfg(any(feature = "jiff", feature = "chrono"))]
        MYSQL_TYPE_TIMESTAMP | MYSQL_TYPE_DATETIME => match ty {
            #[cfg(feature = "jiff")]
            stmt::Type::JiffDateTime => jiff_datetime_or_null(row, i, stmt::Value::JiffDateTime),
            #[cfg(feature = "jiff")]
            stmt::Type::JiffTimestamp => jiff_datetime_or_null(row, i, |dt| {
                stmt::Value::JiffTimestamp(
                    dt.to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                )
            }),
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoDateTimeUtc => extract_or_null(row, i, |v: chrono::NaiveDateTime| {
                stmt::Value::ChronoDateTimeUtc(v.and_utc())
            }),
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoNaiveDateTime => {
                extract_or_null(row, i, stmt::Value::ChronoNaiveDateTime)
            }
            _ => todo!(),
        },

        #[cfg(any(feature = "jiff", feature = "chrono"))]
        MYSQL_TYPE_DATE => match ty {
            #[cfg(feature = "jiff")]
            stmt::Type::JiffDate => match row.take_opt(i).expect("value missing") {
                Ok(mysql_async::Value::Date(year, month, day, _, _, _, _)) => {
                    stmt::Value::JiffDate(jiff::civil::Date::constant(
                        year as i16,
                        month as i8,
                        day as i8,
                    ))
                }
                Ok(mysql_async::Value::NULL) | Err(_) => stmt::Value::Null,
                Ok(v) => panic!("unexpected MySQL value for DATE: {v:#?}"),
            },
            #[cfg(feature = "chrono")]
            stmt::Type::ChronoNaiveDate => extract_or_null(row, i, stmt::Value::ChronoNaiveDate),
            _ => todo!(),
        },

        #[cfg(any(feature = "jiff", feature = "chrono"))]
        MYSQL_TYPE_TIME => {
            match ty {
                #[cfg(feature = "jiff")]
                stmt::Type::JiffTime => {
                    match row.take_opt(i).expect("value missing") {
                        Ok(mysql_async::Value::Time(
                            _is_negative,
                            _days,
                            hour,
                            minute,
                            second,
                            microsecond,
                        )) => {
                            stmt::Value::JiffTime(jiff::civil::Time::constant(
                                hour as i8,
                                minute as i8,
                                second as i8,
                                (microsecond * 1000) as i32, // Convert microseconds to nanoseconds
                            ))
                        }
                        Ok(mysql_async::Value::NULL) | Err(_) => stmt::Value::Null,
                        Ok(v) => panic!("unexpected MySQL value for TIME: {v:#?}"),
                    }
                }
                #[cfg(feature = "chrono")]
                stmt::Type::ChronoNaiveTime => {
                    extract_or_null(row, i, stmt::Value::ChronoNaiveTime)
                }
                _ => todo!(),
            }
        }

        MYSQL_TYPE_NEWDECIMAL | MYSQL_TYPE_DECIMAL => match ty {
            #[cfg(feature = "rust_decimal")]
            stmt::Type::Decimal => extract_or_null(row, i, |s: String| {
                stmt::Value::Decimal(s.parse().expect("failed to parse Decimal from MySQL"))
            }),
            #[cfg(feature = "bigdecimal")]
            stmt::Type::BigDecimal => extract_or_null(row, i, |s: String| {
                stmt::Value::BigDecimal(s.parse().expect("failed to parse BigDecimal from MySQL"))
            }),
            _ => todo!("unexpected type for DECIMAL: {ty:#?}"),
        },

        _ => todo!(
            "implement MySQL to toasty conversion for `{:#?}`; {:#?}; ty={:#?}",
            column.column_type(),
            row.get::<mysql_async::Value, _>(i),
            ty
        ),
    }
}

/// Helper function to extract a value from a MySQL row or return Null if the value is NULL
fn extract_or_null<T>(
    row: &mut mysql_async::Row,
    i: usize,
    constructor: fn(T) -> stmt::Value,
) -> stmt::Value
where
    T: mysql_async::prelude::FromValue,
{
    match row.take_opt(i).expect("value missing") {
        Ok(v) => constructor(v),
        Err(e) => {
            assert!(matches!(e.0, mysql_async::Value::NULL));
            stmt::Value::Null
        }
    }
}

#[cfg(feature = "jiff")]
fn jiff_datetime_or_null(
    row: &mut mysql_async::Row,
    i: usize,
    constructor: fn(jiff::civil::DateTime) -> stmt::Value,
) -> stmt::Value {
    match row.take_opt(i).expect("value missing").unwrap() {
        mysql_async::Value::Date(year, month, day, hour, minute, second, microsecond) => {
            constructor(jiff::civil::DateTime::constant(
                year as i16,
                month as i8,
                day as i8,
                hour as i8,
                minute as i8,
                second as i8,
                (microsecond * 1000) as i32, // Convert microseconds to nanoseconds
            ))
        }
        mysql_async::Value::NULL => stmt::Value::Null,
        v => panic!("unexpected MySQL value for TIMESTAMP/DATETIME: {v:#?}"),
    }
}
