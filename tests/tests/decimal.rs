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

tests!(ty_decimal, ty_decimal_as_text);
