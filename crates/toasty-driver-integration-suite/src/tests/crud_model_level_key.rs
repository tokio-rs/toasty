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
    #[key(part_a, part_b)]
    struct Pair {
        part_a: String,

        part_b: String,
    }

    let mut db = test.setup_db(models!(Pair)).await;

    Pair::create()
        .part_a("hello")
        .part_b("world")
        .exec(&mut db)
        .await?;

    Pair::create()
        .part_a("left")
        .part_b("right")
        .exec(&mut db)
        .await?;

    let found = Pair::filter_by_part_a_and_part_b("hello", "world")
        .get(&mut db)
        .await?;

    assert_eq!(found.part_a, "hello");
    assert_eq!(found.part_b, "world");

    Ok(())
}
