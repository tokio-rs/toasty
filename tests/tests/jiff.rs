use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn ty_timestamp(test: &mut DbTest) {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Timestamp::from_second(946684800).unwrap(), // 2000-01-01T00:00:00Z
        Timestamp::from_second(1609459200).unwrap(), // 2021-01-01T00:00:00Z
        Timestamp::from_second(1735689600).unwrap(), // 2025-01-01T00:00:00Z
        Timestamp::from_second(978307200).unwrap(), // 2001-01-01T00:00:00Z
        Timestamp::from_second(1577836800).unwrap(), // 2020-01-01T00:00:00Z
        Timestamp::from_second(1893456000).unwrap(), // 2030-01-01T00:00:00Z
        Timestamp::from_second(2051222400).unwrap(), // 2035-01-01T00:00:00Z
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_zoned(test: &mut DbTest) {
    use jiff::Zoned;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: Zoned,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        "2000-01-01T00:00:00+00:00[UTC]".parse::<Zoned>().unwrap(),
        "2021-06-15T14:30:00-04:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap(),
        "2025-12-31T23:59:59+09:00[Asia/Tokyo]"
            .parse::<Zoned>()
            .unwrap(),
        "1970-01-01T00:00:00+00:00[UTC]".parse::<Zoned>().unwrap(),
        "2024-11-03T01:30:00-04:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap(), // Before DST fall-back
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_date(test: &mut DbTest) {
    use jiff::civil::Date;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: Date,
    }

    let db = test.setup_db(models!(Foo)).await;

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
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_time(test: &mut DbTest) {
    use jiff::civil::Time;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: Time,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Time::constant(0, 0, 0, 0),
        Time::constant(12, 0, 0, 0),
        Time::constant(23, 59, 59, 999_999_000), // Microsecond precision
        Time::constant(9, 30, 15, 0),
        Time::constant(14, 45, 30, 500_000_000),
        Time::constant(6, 0, 0, 123_456_000), // Microsecond precision
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_datetime(test: &mut DbTest) {
    use jiff::civil::DateTime;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: DateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

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
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_timestamp_precision_2(test: &mut DbTest) {
    use jiff::Timestamp;

    // Skip if database doesn't have native timestamp support
    if !test.capability().native_timestamp {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = timestamp(2))]
        val: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = Timestamp::from_second(946684800)
        .unwrap()
        .checked_add(jiff::Span::new().nanoseconds(123_456_789))
        .unwrap();

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns (truncated to centiseconds)
    let expected = Timestamp::from_second(946684800)
        .unwrap()
        .checked_add(jiff::Span::new().nanoseconds(120_000_000))
        .unwrap();

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_time_precision_2(test: &mut DbTest) {
    use jiff::civil::Time;

    // Skip if database doesn't have native time support
    if !test.capability().native_time {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = time(2))]
        val: Time,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = Time::constant(14, 30, 45, 123_456_789);

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = Time::constant(14, 30, 45, 120_000_000);

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_datetime_precision_2(test: &mut DbTest) {
    use jiff::civil::DateTime;

    // Skip if database doesn't have native datetime support
    if !test.capability().native_datetime {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = datetime(2))]
        val: DateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = DateTime::constant(2024, 6, 15, 14, 30, 45, 123_456_789);

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = DateTime::constant(2024, 6, 15, 14, 30, 45, 120_000_000);

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_timestamp_as_text(test: &mut DbTest) {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Timestamp::from_second(946684800).unwrap(), // 2000-01-01T00:00:00Z
        Timestamp::from_second(1609459200).unwrap(), // 2021-01-01T00:00:00Z
        Timestamp::from_second(1735689600).unwrap(), // 2025-01-01T00:00:00Z
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_date_as_text(test: &mut DbTest) {
    use jiff::civil::Date;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: Date,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Date::constant(2000, 1, 1),
        Date::constant(2021, 6, 15),
        Date::constant(2025, 12, 31),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_time_as_text(test: &mut DbTest) {
    use jiff::civil::Time;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: Time,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Time::constant(0, 0, 0, 0),
        Time::constant(12, 0, 0, 0),
        Time::constant(23, 59, 59, 999_999_000), // Microsecond precision
        Time::constant(9, 30, 15, 0),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_datetime_as_text(test: &mut DbTest) {
    use jiff::civil::DateTime;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: DateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        DateTime::constant(2000, 1, 1, 0, 0, 0, 0),
        DateTime::constant(2021, 6, 15, 14, 30, 0, 0),
        DateTime::constant(2025, 12, 31, 23, 59, 59, 999_999_000), // Microsecond precision
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

tests!(
    ty_timestamp,
    ty_zoned,
    ty_date,
    ty_time,
    ty_datetime,
    ty_timestamp_precision_2,
    ty_time_precision_2,
    ty_datetime_precision_2,
    ty_timestamp_as_text,
    ty_date_as_text,
    ty_time_as_text,
    ty_datetime_as_text
);
