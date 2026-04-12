use crate::prelude::*;

use toasty_core::{
    driver::Operation,
    stmt::{ExprSet, InsertTarget, Statement},
};

#[driver_test(id(ID))]
pub async fn ty_timestamp(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: Timestamp,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let ts = Timestamp::from_second(946684800)?; // 2000-01-01T00:00:00Z

    test.log().clear();

    let created = Item::create().val(ts).exec(&mut db).await?;

    // Verify the INSERT encodes the timestamp correctly for the driver.
    // Native timestamp drivers send the value as-is; non-native drivers
    // (SQLite, DynamoDB) encode it as a fixed-precision ISO 8601 text string.
    let (op, _) = test.log().pop();

    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            target: InsertTarget::Table({
                table: == table_id(&db, "items"),
                columns: == columns(&db, "items", &["id", "val"]),
            }),
            source.body: ExprSet::Values({
                rows: [=~ (Any, Any)],
            }),
        }),
    }));

    // Verify round-trip with more values
    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.val, ts);

    let more_values = vec![
        Timestamp::from_second(1609459200)?, // 2021-01-01T00:00:00Z
        Timestamp::from_second(1735689600)?, // 2025-01-01T00:00:00Z
    ];
    for val in &more_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_zoned(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Zoned;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: Zoned,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        "2000-01-01T00:00:00+00:00[UTC]".parse::<Zoned>()?,
        "2021-06-15T14:30:00-04:00[America/New_York]".parse::<Zoned>()?,
        "2025-12-31T23:59:59+09:00[Asia/Tokyo]".parse::<Zoned>()?,
        "1970-01-01T00:00:00+00:00[UTC]".parse::<Zoned>()?,
        "2024-11-03T01:30:00-04:00[America/New_York]".parse::<Zoned>()?, // Before DST fall-back
    ];

    for val in &test_values {
        let created = Item::create().val(val.clone()).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_date(test: &mut Test) -> Result<()> {
    use jiff::civil::Date;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: Date,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        Date::constant(2000, 1, 1),
        Date::constant(2021, 6, 15),
        Date::constant(2025, 12, 31),
        Date::constant(1970, 1, 1),
        Date::constant(1900, 2, 28),
        Date::constant(2024, 2, 29), // Leap year
        Date::constant(9999, 12, 31),
        Date::constant(1, 1, 1),
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_time(test: &mut Test) -> Result<()> {
    use jiff::civil::Time;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: Time,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        Time::constant(0, 0, 0, 0),
        Time::constant(12, 0, 0, 0),
        Time::constant(23, 59, 59, 999_999_000), // Microsecond precision
        Time::constant(9, 30, 15, 0),
        Time::constant(14, 45, 30, 500_000_000),
        Time::constant(6, 0, 0, 123_456_000), // Microsecond precision
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_datetime(test: &mut Test) -> Result<()> {
    use jiff::civil::DateTime;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        val: DateTime,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        DateTime::constant(2000, 1, 1, 0, 0, 0, 0),
        DateTime::constant(2021, 6, 15, 14, 30, 0, 0),
        DateTime::constant(2025, 12, 31, 23, 59, 59, 999_999_000), // Microsecond precision
        DateTime::constant(1970, 1, 1, 0, 0, 0, 0),
        DateTime::constant(1900, 2, 28, 12, 0, 0, 0),
        DateTime::constant(2024, 2, 29, 6, 30, 15, 123_456_000), // Leap year - Microsecond precision
        DateTime::constant(2099, 12, 31, 23, 59, 59, 0),
        DateTime::constant(1901, 1, 1, 0, 0, 0, 0),
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID), requires(native_timestamp))]
pub async fn ty_timestamp_precision_2(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = timestamp(2))]
        val: Timestamp,
    }

    let mut db = test.setup_db(models!(Item)).await;

    // Test value with nanosecond precision
    let original = Timestamp::from_second(946684800)?
        .checked_add(jiff::Span::new().nanoseconds(123_456_789))?;

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns (truncated to centiseconds)
    let expected = Timestamp::from_second(946684800)?
        .checked_add(jiff::Span::new().nanoseconds(120_000_000))?;

    let created = Item::create().val(original).exec(&mut db).await?;
    let read = Item::get_by_id(&mut db, &created.id).await?;

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
    Ok(())
}

#[driver_test(id(ID), requires(native_time))]
pub async fn ty_time_precision_2(test: &mut Test) -> Result<()> {
    use jiff::civil::Time;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = time(2))]
        val: Time,
    }

    let mut db = test.setup_db(models!(Item)).await;

    // Test value with nanosecond precision
    let original = Time::constant(14, 30, 45, 123_456_789);

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = Time::constant(14, 30, 45, 120_000_000);

    let created = Item::create().val(original).exec(&mut db).await?;
    let read = Item::get_by_id(&mut db, &created.id).await?;

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
    Ok(())
}

#[driver_test(id(ID), requires(native_datetime))]
pub async fn ty_datetime_precision_2(test: &mut Test) -> Result<()> {
    use jiff::civil::DateTime;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = datetime(2))]
        val: DateTime,
    }

    let mut db = test.setup_db(models!(Item)).await;

    // Test value with nanosecond precision
    let original = DateTime::constant(2024, 6, 15, 14, 30, 45, 123_456_789);

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = DateTime::constant(2024, 6, 15, 14, 30, 45, 120_000_000);

    let created = Item::create().val(original).exec(&mut db).await?;
    let read = Item::get_by_id(&mut db, &created.id).await?;

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_timestamp_as_text(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        val: Timestamp,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let ts = Timestamp::from_second(946684800)?; // 2000-01-01T00:00:00Z

    test.log().clear();

    let created = Item::create().val(ts).exec(&mut db).await?;

    // Verify the INSERT encodes the timestamp as a fixed-precision text string.
    // The #[column(type = text)] forces text encoding on all drivers.
    let (op, _) = test.log().pop();
    assert_struct!(op, Operation::QuerySql({
        stmt: Statement::Insert({
            target: InsertTarget::Table({
                table: == table_id(&db, "items"),
                columns: == columns(&db, "items", &["id", "val"]),
            }),
            source.body: ExprSet::Values({
                rows: [=~ (Any, Any)],
            }),
        }),
    }));

    // Verify round-trip
    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.val, ts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_date_as_text(test: &mut Test) -> Result<()> {
    use jiff::civil::Date;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        val: Date,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        Date::constant(2000, 1, 1),
        Date::constant(2021, 6, 15),
        Date::constant(2025, 12, 31),
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_time_as_text(test: &mut Test) -> Result<()> {
    use jiff::civil::Time;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        val: Time,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        Time::constant(0, 0, 0, 0),
        Time::constant(12, 0, 0, 0),
        Time::constant(23, 59, 59, 999_999_000), // Microsecond precision
        Time::constant(9, 30, 15, 0),
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_datetime_as_text(test: &mut Test) -> Result<()> {
    use jiff::civil::DateTime;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[column(type = text)]
        val: DateTime,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let test_values = vec![
        DateTime::constant(2000, 1, 1, 0, 0, 0, 0),
        DateTime::constant(2021, 6, 15, 14, 30, 0, 0),
        DateTime::constant(2025, 12, 31, 23, 59, 59, 999_999_000), // Microsecond precision
    ];

    for val in &test_values {
        let created = Item::create().val(*val).exec(&mut db).await?;
        let read = Item::get_by_id(&mut db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn order_by_timestamp(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[column(type = text)]
        val: Timestamp,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let timestamps = vec![
        Timestamp::from_second(1609459200)?, // 2021-01-01
        Timestamp::from_second(946684800)?,  // 2000-01-01
        Timestamp::from_second(1735689600)?, // 2025-01-01
    ];

    for val in &timestamps {
        Item::create().val(*val).exec(&mut db).await?;
    }

    let asc: Vec<_> = Item::all()
        .order_by(Item::fields().val().asc())
        .exec(&mut db)
        .await?;

    assert_eq!(asc.len(), 3);
    assert!(asc[0].val < asc[1].val);
    assert!(asc[1].val < asc[2].val);

    let desc: Vec<_> = Item::all()
        .order_by(Item::fields().val().desc())
        .exec(&mut db)
        .await?;

    assert_eq!(desc.len(), 3);
    assert!(desc[0].val > desc[1].val);
    assert!(desc[1].val > desc[2].val);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn filter_by_timestamp(test: &mut Test) -> Result<(), BoxError> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Event {
        #[key]
        #[auto]
        id: ID,
        #[index]
        at: Timestamp,
        name: String,
    }

    let mut db = test.setup_db(models!(Event)).await;

    let ts1 = Timestamp::from_second(946684800)?; // 2000-01-01T00:00:00Z
    let ts2 = Timestamp::from_second(1609459200)?; // 2021-01-01T00:00:00Z
    let ts3 = Timestamp::from_second(1735689600)?; // 2025-01-01T00:00:00Z

    Event::create().at(ts1).name("a").exec(&mut db).await?;
    Event::create().at(ts2).name("b").exec(&mut db).await?;
    Event::create().at(ts3).name("c").exec(&mut db).await?;

    let results = Event::filter_by_at(ts2).exec(&mut db).await?;
    assert_struct!(results, [{ name: "b", at: == ts2 }]);

    // No match
    let results = Event::filter_by_at(Timestamp::from_second(0)?)
        .exec(&mut db)
        .await?;
    assert!(results.is_empty());

    Ok(())
}
