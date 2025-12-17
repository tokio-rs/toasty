use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn ty_decimal(test: &mut DbTest) {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        val: Decimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("1").unwrap(),
        Decimal::from_str("-1").unwrap(),
        Decimal::from_str("123.456").unwrap(),
        Decimal::from_str("-123.456").unwrap(),
        Decimal::from_str("0.0000000001").unwrap(), // Small positive
        Decimal::from_str("-0.0000000001").unwrap(), // Small negative
        Decimal::from_str("99999999999999999999.99999999").unwrap(), // Large with precision
        Decimal::from_str("-99999999999999999999.99999999").unwrap(), // Large negative with precision
        Decimal::from_str("3.141592653589793238").unwrap(),           // Pi approximation
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_decimal_as_text(test: &mut DbTest) {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = text)]
        val: Decimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("123.456").unwrap(),
        Decimal::from_str("-123.456").unwrap(),
        Decimal::from_str("99999999999999999999.99999999").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_decimal_as_numeric_arbitrary_precision(test: &mut DbTest) {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // Only test on databases that support arbitrary precision decimals
    if !test.capability().storage_types.decimal_arbitrary_precision {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = numeric)]
        val: Decimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("123.456").unwrap(),
        Decimal::from_str("-123.456").unwrap(),
        Decimal::from_str("99999999999999999999.99999999").unwrap(),
        Decimal::from_str("3.141592653589793238").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

async fn ty_decimal_as_numeric_fixed_precision(test: &mut DbTest) {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // Only test on databases that support native decimal types
    if !test.capability().storage_types.native_decimal {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
        #[column(type = numeric(28, 10))]
        val: Decimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        Decimal::from_str("0").unwrap(),
        Decimal::from_str("123.456").unwrap(),
        Decimal::from_str("-123.456").unwrap(),
        Decimal::from_str("123456789012345678.1234567890").unwrap(), // Within precision(28,10)
        Decimal::from_str("3.1415926535").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

tests!(
    ty_decimal,
    ty_decimal_as_text,
    ty_decimal_as_numeric_arbitrary_precision,
    ty_decimal_as_numeric_fixed_precision
);
