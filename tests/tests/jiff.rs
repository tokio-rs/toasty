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
        Timestamp::UNIX_EPOCH,
        Timestamp::from_second(0).unwrap(),
        Timestamp::from_second(946684800).unwrap(), // 2000-01-01T00:00:00Z
        Timestamp::from_second(1609459200).unwrap(), // 2021-01-01T00:00:00Z
        Timestamp::from_second(1735689600).unwrap(), // 2025-01-01T00:00:00Z
        Timestamp::from_second(-62135596800).unwrap(), // 0001-01-01T00:00:00Z
        Timestamp::from_second(253402300799).unwrap(), // 9999-12-31T23:59:59Z
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
        "2000-01-01T00:00:00Z".parse::<Zoned>().unwrap(),
        "2021-06-15T14:30:00-04:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap(),
        "2025-12-31T23:59:59+09:00[Asia/Tokyo]"
            .parse::<Zoned>()
            .unwrap(),
        "1970-01-01T00:00:00Z".parse::<Zoned>().unwrap(),
        "2024-03-10T02:30:00-05:00[America/New_York]"
            .parse::<Zoned>()
            .unwrap(), // DST transition
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
        Time::constant(23, 59, 59, 999_999_999),
        Time::constant(9, 30, 15, 0),
        Time::constant(14, 45, 30, 500_000_000),
        Time::constant(6, 0, 0, 123_456_789),
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
        DateTime::constant(2025, 12, 31, 23, 59, 59, 999_999_999),
        DateTime::constant(1970, 1, 1, 0, 0, 0, 0),
        DateTime::constant(1900, 2, 28, 12, 0, 0, 0),
        DateTime::constant(2024, 2, 29, 6, 30, 15, 123_456_789), // Leap year
        DateTime::constant(9999, 12, 31, 23, 59, 59, 0),
        DateTime::constant(1, 1, 1, 0, 0, 0, 0),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

tests!(ty_timestamp, ty_zoned, ty_date, ty_time, ty_datetime);
