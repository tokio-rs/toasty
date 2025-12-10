use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn ty_datetime_utc(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: DateTime<Utc>,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap(), // 2000-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(), // 2021-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(), // 2025-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2001, 1, 1, 0, 0, 0).unwrap(), // 2001-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap(), // 2020-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap(), // 2030-01-01T00:00:00Z
        Utc.with_ymd_and_hms(2035, 1, 1, 0, 0, 0).unwrap(), // 2035-01-01T00:00:00Z
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivedate(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: NaiveDate,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2021, 6, 15).unwrap(),
        NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(1900, 2, 28).unwrap(),
        NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(), // Leap year
        NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(),
        NaiveDate::from_ymd_opt(1000, 1, 1).unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivetime(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: NaiveTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveTime::from_hms_nano_opt(0, 0, 0, 0).unwrap(),
        NaiveTime::from_hms_nano_opt(12, 0, 0, 0).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_000).unwrap(), // Microsecond precision
        NaiveTime::from_hms_nano_opt(9, 30, 15, 0).unwrap(),
        NaiveTime::from_hms_nano_opt(14, 45, 30, 500_000_000).unwrap(),
        NaiveTime::from_hms_nano_opt(6, 0, 0, 123_456_000).unwrap(), // Microsecond precision
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivedatetime(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: NaiveDateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveDate::from_ymd_opt(2000, 1, 1)
            .unwrap()
            .and_hms_nano_opt(0, 0, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(2021, 6, 15)
            .unwrap()
            .and_hms_nano_opt(14, 30, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(2025, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 999_999_000)
            .unwrap(), // Microsecond precision
        NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .and_hms_nano_opt(0, 0, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(1900, 2, 28)
            .unwrap()
            .and_hms_nano_opt(12, 0, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(2024, 2, 29)
            .unwrap()
            .and_hms_nano_opt(6, 30, 15, 123_456_000)
            .unwrap(), // Leap year - Microsecond precision
        NaiveDate::from_ymd_opt(2099, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(1901, 1, 1)
            .unwrap()
            .and_hms_nano_opt(0, 0, 0, 0)
            .unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_datetimeutc_precision_2(test: &mut DbTest) {
    // Skip if database doesn't have native timestamp support
    if !test.capability().storage_types.native_timestamp {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = timestamp(2))]
        val: DateTime<Utc>,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = Utc.timestamp_opt(946684800, 123_456_789).unwrap();

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns (truncated to centiseconds)
    let expected = Utc.timestamp_opt(946684800, 120_000_000).unwrap();

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_naivetime_precision_2(test: &mut DbTest) {
    // Skip if database doesn't have native time support
    if !test.capability().storage_types.native_time {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = time(2))]
        val: NaiveTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = NaiveTime::from_hms_nano_opt(14, 30, 45, 123_456_789).unwrap();

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = NaiveTime::from_hms_nano_opt(14, 30, 45, 120_000_000).unwrap();

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_naivedatetime_precision_2(test: &mut DbTest) {
    // Skip if database doesn't have native datetime support
    if !test.capability().storage_types.native_datetime {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = datetime(2))]
        val: NaiveDateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Test value with nanosecond precision
    let original = NaiveDate::from_ymd_opt(2024, 6, 15)
        .unwrap()
        .and_hms_nano_opt(14, 30, 45, 123_456_789)
        .unwrap();

    // Expected value truncated to 2 decimal places (centiseconds = 10ms precision)
    // 123_456_789 ns -> 120_000_000 ns
    let expected = NaiveDate::from_ymd_opt(2024, 6, 15)
        .unwrap()
        .and_hms_nano_opt(14, 30, 45, 120_000_000)
        .unwrap();

    let created = Foo::create().val(original).exec(&db).await.unwrap();
    let read = Foo::get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(
        read.val, expected,
        "Precision truncation failed: original={}, read={}, expected={}",
        original, read.val, expected
    );
}

async fn ty_datetimeutc_as_text(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: DateTime<Utc>,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Utc.timestamp_opt(946684800, 0).unwrap(), // 2000-01-01T00:00:00Z
        Utc.timestamp_opt(1609459200, 0).unwrap(), // 2021-01-01T00:00:00Z
        Utc.timestamp_opt(1735689600, 0).unwrap(), // 2025-01-01T00:00:00Z
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivedate_as_text(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: NaiveDate,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2021, 6, 15).unwrap(),
        NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivetime_as_text(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: NaiveTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveTime::from_hms_nano_opt(0, 0, 0, 0).unwrap(),
        NaiveTime::from_hms_nano_opt(12, 0, 0, 0).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_000).unwrap(), // Microsecond precision
        NaiveTime::from_hms_nano_opt(9, 30, 15, 0).unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_naivedatetime_as_text(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: NaiveDateTime,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        NaiveDate::from_ymd_opt(2000, 1, 1)
            .unwrap()
            .and_hms_nano_opt(0, 0, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(2021, 6, 15)
            .unwrap()
            .and_hms_nano_opt(14, 30, 0, 0)
            .unwrap(),
        NaiveDate::from_ymd_opt(2025, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 999_999_000)
            .unwrap(), // Microsecond precision
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

tests!(
    ty_datetime_utc,
    ty_naivedate,
    ty_naivetime,
    ty_naivedatetime,
    ty_datetimeutc_precision_2,
    ty_naivetime_precision_2,
    ty_naivedatetime_precision_2,
    ty_datetimeutc_as_text,
    ty_naivedate_as_text,
    ty_naivetime_as_text,
    ty_naivedatetime_as_text,
);
