use crate::prelude::*;

use rust_decimal::Decimal;
use std::str::FromStr;

#[driver_test(id(ID))]
pub async fn ty_decimal(test: &mut Test) -> Result<(), BoxError> {
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
        Decimal::from_str("0")?,
        Decimal::from_str("1")?,
        Decimal::from_str("-1")?,
        Decimal::from_str("123.456")?,
        Decimal::from_str("-123.456")?,
        Decimal::from_str("0.0000000001")?,  // Small positive
        Decimal::from_str("-0.0000000001")?, // Small negative
        Decimal::from_str("99999999999999999999.99999999")?, // Large with precision
        Decimal::from_str("-99999999999999999999.99999999")?, // Large negative with precision
        Decimal::from_str("3.141592653589793238")?, // Pi approximation
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_decimal_as_text(test: &mut Test) -> Result<(), BoxError> {
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
        Decimal::from_str("0")?,
        Decimal::from_str("123.456")?,
        Decimal::from_str("-123.456")?,
        Decimal::from_str("99999999999999999999.99999999")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID), requires(decimal_arbitrary_precision))]
pub async fn ty_decimal_as_numeric_arbitrary_precision(test: &mut Test) -> Result<(), BoxError> {
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
        Decimal::from_str("0")?,
        Decimal::from_str("123.456")?,
        Decimal::from_str("-123.456")?,
        Decimal::from_str("99999999999999999999.99999999")?,
        Decimal::from_str("3.141592653589793238")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID), requires(native_decimal))]
pub async fn ty_decimal_as_numeric_fixed_precision(test: &mut Test) -> Result<(), BoxError> {
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
        Decimal::from_str("0")?,
        Decimal::from_str("123.456")?,
        Decimal::from_str("-123.456")?,
        Decimal::from_str("123456789012345678.1234567890")?, // Within precision(28,10)
        Decimal::from_str("3.1415926535")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(*val).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}
