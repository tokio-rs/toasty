use crate::prelude::*;

/// Test that `#[key(field)]` on the model is equivalent to `#[key]` on the field
#[driver_test]
pub async fn model_level_single_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(name)]
    struct Widget {
        name: String,

        value: i64,
    }

    let mut db = test.setup_db(models!(Widget)).await;

    let widget = Widget::create()
        .name("sprocket")
        .value(42)
        .exec(&mut db)
        .await?;

    assert_eq!(widget.name, "sprocket");
    assert_eq!(widget.value, 42);

    // Find by key
    let found = Widget::filter_by_name("sprocket").get(&mut db).await?;
    assert_eq!(found.value, 42);

    Ok(())
}

/// Test that `#[key(a, b)]` on the model is equivalent to `#[key]` on both fields
#[driver_test]
pub async fn model_level_composite_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(one, two)]
    struct Bar {
        one: String,

        two: String,
    }

    let mut db = test.setup_db(models!(Bar)).await;

    Bar::create()
        .one("hello")
        .two("world")
        .exec(&mut db)
        .await?;

    Bar::create().one("foo").two("bar").exec(&mut db).await?;

    let found = Bar::filter_by_one_and_two("hello", "world")
        .get(&mut db)
        .await?;

    assert_eq!(found.one, "hello");
    assert_eq!(found.two, "world");

    Ok(())
}
