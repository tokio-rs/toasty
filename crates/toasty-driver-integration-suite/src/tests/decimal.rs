use crate::prelude::*;

use rust_decimal::Decimal;
use std::str::FromStr;

#[driver_test(id(ID))]
pub async fn ty_decimal(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
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

#[driver_test(id(ID))]
pub async fn ty_decimal_as_text(test: &mut Test) {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
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

#[driver_test(id(ID), requires(decimal_arbitrary_precision))]
pub async fn ty_decimal_as_numeric_arbitrary_precision(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
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

#[driver_test(id(ID), requires(native_decimal))]
pub async fn ty_decimal_as_numeric_fixed_precision(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
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
