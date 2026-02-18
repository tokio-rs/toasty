use crate::prelude::*;

use bigdecimal::BigDecimal;
use std::str::FromStr;

#[driver_test(id(ID))]
pub async fn ty_bigdecimal(test: &mut Test) -> Result<(), BoxError> {
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
        BigDecimal::from_str("0")?,
        BigDecimal::from_str("1")?,
        BigDecimal::from_str("-1")?,
        BigDecimal::from_str("123.456")?,
        BigDecimal::from_str("-123.456")?,
        BigDecimal::from_str("0.0000000001")?, // Small positive
        BigDecimal::from_str("-0.0000000001")?, // Small negative
        BigDecimal::from_str("99999999999999999999.99999999999999999999")?, // Large with precision
        BigDecimal::from_str("-99999999999999999999.99999999999999999999")?, // Large negative with precision
        BigDecimal::from_str("3.141592653589793238462643383279")?,           // Pi approximation
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn ty_bigdecimal_as_text(test: &mut Test) -> Result<(), BoxError> {
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
        BigDecimal::from_str("0")?,
        BigDecimal::from_str("123.456")?,
        BigDecimal::from_str("-123.456")?,
        BigDecimal::from_str("99999999999999999999.99999999999999999999")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(
    id(ID),
    requires(and(bigdecimal_implemented, decimal_arbitrary_precision))
)]
pub async fn ty_bigdecimal_as_numeric_arbitrary_precision(test: &mut Test) -> Result<(), BoxError> {
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
        BigDecimal::from_str("0")?,
        BigDecimal::from_str("123.456")?,
        BigDecimal::from_str("-123.456")?,
        BigDecimal::from_str("99999999999999999999.99999999999999999999")?,
        BigDecimal::from_str("3.141592653589793238462643383279")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}

#[driver_test(id(ID), requires(and(native_decimal, bigdecimal_implemented)))]
pub async fn ty_bigdecimal_as_numeric_fixed_precision(test: &mut Test) -> Result<(), BoxError> {
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
        BigDecimal::from_str("0")?,
        BigDecimal::from_str("123.456")?,
        BigDecimal::from_str("-123.456")?,
        BigDecimal::from_str("123456789012345678.12345678901234567890")?, // Within precision(38,20)
        BigDecimal::from_str("3.14159265358979323846")?,
    ];

    for val in &test_values {
        let created = Foo::create().val(val.clone()).exec(&db).await?;
        let read = Foo::get_by_id(&db, &created.id).await?;
        assert_eq!(read.val, *val, "Round-trip failed for: {}", val);
    }
    Ok(())
}
