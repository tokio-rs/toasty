use crate::prelude::*;

use toasty::stmt::{self, Value};
use toasty_core::{
    driver::{Operation, operation::RawSqlRet},
    schema::db,
};

#[driver_test(requires(sql))]
pub async fn statement_and_query_on_db(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let count = toasty::sql::statement(format!(
        "UPDATE {table} SET name = {} WHERE name = {}",
        placeholder(&db, 1),
        placeholder(&db, 2),
    ))
    .bind("Bob")
    .bind("Alice")
    .exec(&mut db)
    .await?;
    assert_eq!(count, 1);

    let rows = toasty::sql::query(format!("SELECT name FROM {table}"))
        .exec(&mut db)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &toasty::stmt::Value::from("Bob"));

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn query_on_connection(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let mut conn = db.connection().await?;
    let rows = toasty::sql::query(format!(
        "SELECT name FROM {table} WHERE name = {}",
        placeholder(&db, 1),
    ))
    .bind("Alice")
    .exec(&mut conn)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &toasty::stmt::Value::from("Alice"));

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn statement_inside_transaction_rolls_back(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let table = table_name(&db, "users");

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    let mut tx = db.transaction().await?;
    toasty::sql::statement(format!("UPDATE {table} SET name = 'Bob'"))
        .exec(&mut tx)
        .await?;
    tx.rollback().await?;

    let user = User::filter_by_name("Alice").get(&mut db).await?;
    assert_eq!(user.name, "Alice");

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn query_is_sent_as_raw_sql_driver_operation(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let sql = format!("SELECT {}", placeholder(&db, 1));

    t.log().clear();
    let rows = toasty::sql::query(sql.clone())
        .bind_typed(Value::I64(123), db::Type::Integer(8))
        .column_types([stmt::Type::I64])
        .exec(&mut db)
        .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &Value::I64(123));

    let (op, _) = t.log().pop();
    let Operation::RawSql(op) = op else {
        panic!("expected RawSql operation, got {op:?}");
    };

    assert_eq!(op.sql, sql);
    assert_eq!(op.params.len(), 1);
    assert_eq!(op.params[0].value, Value::I64(123));
    assert_eq!(op.params[0].ty, db::Type::Integer(8));
    assert!(matches!(op.ret, RawSqlRet::Types(types) if types == vec![stmt::Type::I64]));
    assert!(t.log().is_empty());

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn query_infers_storage_values(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        bool_v: bool,
        int_v: i64,
        float_v: f64,
        string_v: String,
        bytes_v: Vec<u8>,
        uuid_v: uuid::Uuid,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let uuid = uuid::Uuid::from_u128(0x0123456789abcdef0123456789abcdef);

    let item = toasty::create!(Item {
        bool_v: true,
        int_v: -42,
        float_v: 10.25,
        string_v: "hello".to_string(),
        bytes_v: vec![0, 1, 2, 255],
        uuid_v: uuid,
    })
    .exec(&mut db)
    .await?;

    let rows = toasty::sql::query(format!(
        "SELECT bool_v, int_v, float_v, string_v, bytes_v, uuid_v FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 1);

    let expected_bool = if is_postgresql(&db) {
        Value::Bool(true)
    } else {
        Value::I64(1)
    };
    let expected_uuid = match db.capability().sql_placeholder.unwrap() {
        toasty::SqlPlaceholder::DollarNumber => Value::Uuid(uuid),
        toasty::SqlPlaceholder::QuestionMark => Value::String(uuid.to_string()),
        toasty::SqlPlaceholder::NumberedQuestionMark => Value::Bytes(uuid.as_bytes().to_vec()),
    };
    let expected = vec![
        expected_bool,
        Value::I64(-42),
        Value::F64(10.25),
        Value::String("hello".to_string()),
        Value::Bytes(vec![0, 1, 2, 255]),
        expected_uuid,
    ];

    assert_eq!(fields(&rows[0]), expected.as_slice());

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn query_column_types_decode_scalars(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        bool_v: bool,
        i8_v: i8,
        i16_v: i16,
        i32_v: i32,
        i64_v: i64,
        u8_v: u8,
        u16_v: u16,
        u32_v: u32,
        u64_v: u64,
        f32_v: f32,
        f64_v: f64,
        string_v: String,
        bytes_v: Vec<u8>,
        #[column(type = text)]
        uuid_v: uuid::Uuid,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let uuid = uuid::Uuid::from_u128(0xfedcba9876543210fedcba9876543210);

    let item = toasty::create!(Item {
        bool_v: true,
        i8_v: -8,
        i16_v: -16,
        i32_v: -32,
        i64_v: -64,
        u8_v: 8,
        u16_v: 16,
        u32_v: 32,
        u64_v: 64,
        f32_v: 1.25,
        f64_v: 2.5,
        string_v: "hello".to_string(),
        bytes_v: vec![3, 4, 5, 255],
        uuid_v: uuid,
    })
    .exec(&mut db)
    .await?;

    let rows = toasty::sql::query(format!(
        "SELECT bool_v, i8_v, i16_v, i32_v, i64_v, u8_v, u16_v, u32_v, u64_v, \
         f32_v, f64_v, string_v, bytes_v, uuid_v FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .column_types([
        stmt::Type::Bool,
        stmt::Type::I8,
        stmt::Type::I16,
        stmt::Type::I32,
        stmt::Type::I64,
        stmt::Type::U8,
        stmt::Type::U16,
        stmt::Type::U32,
        stmt::Type::U64,
        stmt::Type::F32,
        stmt::Type::F64,
        stmt::Type::String,
        stmt::Type::Bytes,
        stmt::Type::Uuid,
    ])
    .exec(&mut db)
    .await?;

    let expected = vec![
        Value::Bool(true),
        Value::I8(-8),
        Value::I16(-16),
        Value::I32(-32),
        Value::I64(-64),
        Value::U8(8),
        Value::U16(16),
        Value::U32(32),
        Value::U64(64),
        Value::F32(1.25),
        Value::F64(2.5),
        Value::String("hello".to_string()),
        Value::Bytes(vec![3, 4, 5, 255]),
        Value::Uuid(uuid),
    ];

    assert_eq!(rows.len(), 1);
    assert_eq!(fields(&rows[0]), expected.as_slice());

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn statement_binds_typed_null(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        name: Option<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");

    let count = toasty::sql::statement(format!(
        "INSERT INTO {table} (name) VALUES ({})",
        placeholder(&db, 1),
    ))
    .bind_typed(Value::Null, toasty::schema::db::Type::Text)
    .exec(&mut db)
    .await?;
    assert_eq!(count, 1);

    let rows = toasty::sql::query(format!("SELECT name FROM {table}"))
        .exec(&mut db)
        .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &Value::Null);

    Ok(())
}

#[driver_test(requires(and(sql, vec_scalar)))]
pub async fn query_column_types_decode_list(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let tags = vec!["rust".to_string(), "toasty".to_string()];

    let item = toasty::create!(Item { tags: tags.clone() })
        .exec(&mut db)
        .await?;

    let rows = toasty::sql::query(format!(
        "SELECT tags FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .column_types([stmt::Type::list(stmt::Type::String)])
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(
        field(&rows[0], 0),
        &Value::List(tags.into_iter().map(Value::String).collect())
    );

    Ok(())
}

#[driver_test(requires(and(sql, native_datetime)))]
pub async fn query_column_types_decode_temporal(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        timestamp_v: jiff::Timestamp,
        date_v: jiff::civil::Date,
        time_v: jiff::civil::Time,
        datetime_v: jiff::civil::DateTime,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let timestamp = jiff::Timestamp::from_second(1_700_000_000)?
        .checked_add(jiff::Span::new().nanoseconds(123_456_000))?;
    let date = jiff::civil::date(2025, 6, 15);
    let time = jiff::civil::time(9, 30, 45, 123_456_000);
    let datetime = jiff::civil::datetime(2025, 6, 15, 9, 30, 45, 123_456_000);

    let item = toasty::create!(Item {
        timestamp_v: timestamp,
        date_v: date,
        time_v: time,
        datetime_v: datetime,
    })
    .exec(&mut db)
    .await?;

    let rows = toasty::sql::query(format!(
        "SELECT timestamp_v, date_v, time_v, datetime_v FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .column_types([
        stmt::Type::Timestamp,
        stmt::Type::Date,
        stmt::Type::Time,
        stmt::Type::DateTime,
    ])
    .exec(&mut db)
    .await?;

    let expected = vec![
        Value::Timestamp(timestamp),
        Value::Date(date),
        Value::Time(time),
        Value::DateTime(datetime),
    ];

    assert_eq!(rows.len(), 1);
    assert_eq!(fields(&rows[0]), expected.as_slice());

    Ok(())
}

#[driver_test(requires(and(sql, native_decimal)))]
pub async fn query_column_types_decode_decimal(t: &mut Test) -> Result<(), BoxError> {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        #[column(type = numeric(28, 10))]
        val: Decimal,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let val = Decimal::from_str("123456789012345678.1234567890")?;

    let item = toasty::create!(Item { val }).exec(&mut db).await?;

    let rows = toasty::sql::query(format!(
        "SELECT val FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .column_types([stmt::Type::Decimal])
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &Value::Decimal(val));

    Ok(())
}

#[driver_test(requires(and(sql, native_decimal, bigdecimal_implemented)))]
pub async fn query_column_types_decode_bigdecimal(t: &mut Test) -> Result<(), BoxError> {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: u64,
        #[column(type = numeric(38, 20))]
        val: BigDecimal,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let table = table_name(&db, "items");
    let val = BigDecimal::from_str("123456789012345678.12345678901234567890")?;

    let item = toasty::create!(Item { val: val.clone() })
        .exec(&mut db)
        .await?;

    let rows = toasty::sql::query(format!(
        "SELECT val FROM {table} WHERE id = {}",
        placeholder(&db, 1),
    ))
    .bind(item.id)
    .column_types([stmt::Type::BigDecimal])
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(field(&rows[0], 0), &Value::BigDecimal(val));

    Ok(())
}

#[driver_test(requires(not(sql)))]
pub async fn raw_sql_rejects_non_sql_drivers(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: uuid::Uuid,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let err = toasty::sql::statement("SELECT 1")
        .exec(&mut db)
        .await
        .unwrap_err();

    assert!(
        err.is_unsupported_feature(),
        "expected UnsupportedFeature, got: {err}"
    );

    Ok(())
}

fn table_name(db: &toasty::Db, suffix: &str) -> String {
    db.schema()
        .db
        .tables
        .iter()
        .find(|table| table.name.ends_with(suffix))
        .map(|table| table.name.clone())
        .unwrap_or_else(|| panic!("table {suffix} not found"))
}

fn placeholder(db: &toasty::Db, index: usize) -> String {
    render_placeholder(
        db.capability()
            .sql_placeholder
            .expect("SQL driver has placeholders"),
        index,
    )
}

fn render_placeholder(placeholder: toasty::SqlPlaceholder, index: usize) -> String {
    match placeholder {
        toasty::SqlPlaceholder::QuestionMark => "?".to_string(),
        toasty::SqlPlaceholder::NumberedQuestionMark => format!("?{index}"),
        toasty::SqlPlaceholder::DollarNumber => format!("${index}"),
    }
}

fn is_postgresql(db: &toasty::Db) -> bool {
    matches!(
        db.capability().sql_placeholder,
        Some(toasty::SqlPlaceholder::DollarNumber)
    )
}

fn fields(value: &Value) -> &[Value] {
    let Value::Record(record) = value else {
        panic!("expected record, got {value:?}");
    };

    record
}

fn field(value: &toasty::stmt::Value, index: usize) -> &toasty::stmt::Value {
    &fields(value)[index]
}
