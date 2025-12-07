use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn ty_bigdecimal(test: &mut DbTest) {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
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

async fn ty_bigdecimal_as_text(test: &mut DbTest) {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
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

async fn ty_bigdecimal_as_numeric_arbitrary_precision(test: &mut DbTest) {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    // Only test on databases that support arbitrary precision decimals
    // and have BigDecimal driver support implemented
    if !test.capability().storage_types.decimal_arbitrary_precision
        || !test.capability().bigdecimal_implemented
    {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
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

async fn ty_bigdecimal_as_numeric_fixed_precision(test: &mut DbTest) {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    // Only test on databases that support native decimal types
    // Skip PostgreSQL as BigDecimal support is not yet implemented
    if !test.capability().storage_types.native_decimal || !test.capability().bigdecimal_implemented
    {
        return;
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Foo {
        #[key]
        #[auto]
        id: Id<Self>,
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

tests!(
    ty_bigdecimal,
    ty_bigdecimal_as_text,
    ty_bigdecimal_as_numeric_arbitrary_precision,
    ty_bigdecimal_as_numeric_fixed_precision
);
