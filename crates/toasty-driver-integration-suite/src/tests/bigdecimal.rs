use crate::prelude::*;

use bigdecimal::BigDecimal;
use std::str::FromStr;

#[driver_test(id(ID))]
pub async fn ty_bigdecimal(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
        val: BigDecimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        BigDecimal::from_str("0").unwrap(),
        BigDecimal::from_str("1").unwrap(),
        BigDecimal::from_str("-1").unwrap(),
        BigDecimal::from_str("123.456").unwrap(),
        BigDecimal::from_str("-123.456").unwrap(),
        BigDecimal::from_str("0.0000000001").unwrap(), // Small positive
        BigDecimal::from_str("-0.0000000001").unwrap(), // Small negative
        BigDecimal::from_str("99999999999999999999.99999999999999999999").unwrap(), // Large with precision
        BigDecimal::from_str("-99999999999999999999.99999999999999999999").unwrap(), // Large negative with precision
        BigDecimal::from_str("3.141592653589793238462643383279").unwrap(), // Pi approximation
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

#[driver_test(id(ID))]
pub async fn ty_bigdecimal_as_text(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[column(type = text)]
        val: BigDecimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        BigDecimal::from_str("0").unwrap(),
        BigDecimal::from_str("123.456").unwrap(),
        BigDecimal::from_str("-123.456").unwrap(),
        BigDecimal::from_str("99999999999999999999.99999999999999999999").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

#[driver_test(id(ID), requires(bigdecimal_implemented, decimal_arbitrary_precision))]
pub async fn ty_bigdecimal_as_numeric_arbitrary_precision(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
        #[column(type = numeric)]
        val: BigDecimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        BigDecimal::from_str("0").unwrap(),
        BigDecimal::from_str("123.456").unwrap(),
        BigDecimal::from_str("-123.456").unwrap(),
        BigDecimal::from_str("99999999999999999999.99999999999999999999").unwrap(),
        BigDecimal::from_str("3.141592653589793238462643383279").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}

#[driver_test(id(ID), requires(native_decimal, bigdecimal_implemented))]
pub async fn ty_bigdecimal_as_numeric_fixed_precision(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
        #[column(type = numeric(38, 20))]
        val: BigDecimal,
    }

    let db = test.setup_db(models!(Foo)).await;

    let test_values = vec![
        BigDecimal::from_str("0").unwrap(),
        BigDecimal::from_str("123.456").unwrap(),
        BigDecimal::from_str("-123.456").unwrap(),
        BigDecimal::from_str("123456789012345678.12345678901234567890").unwrap(), // Within precision(38,20)
        BigDecimal::from_str("3.14159265358979323846").unwrap(),
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await.unwrap();
        let read = Foo::get_by_id(&db, &created.id).await.unwrap();
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
}
