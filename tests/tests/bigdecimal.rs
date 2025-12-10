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

tests!(ty_bigdecimal, ty_bigdecimal_as_text);
