use crate::prelude::*;

#[driver_test]
pub async fn filter_composite_key_in_list(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for i in 0..5 {
        Item::create()
            .one(format!("one-{i}"))
            .two(format!("two-{i}"))
            .exec(&mut db)
            .await?;
    }

    // Use the free function form with a tuple of field paths
    let items: Vec<_> = Item::filter(toasty::stmt::in_list::<(String, String)>(
        (Item::fields().one(), Item::fields().two()),
        [("one-1", "two-1"), ("one-3", "two-3")],
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 2);

    for item in &items {
        assert!(
            (item.one == "one-1" && item.two == "two-1")
                || (item.one == "one-3" && item.two == "two-3")
        );
    }

    Ok(())
}

#[driver_test]
pub async fn filter_composite_key_in_list_empty(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let mut db = test.setup_db(models!(Item)).await;

    Item::create().one("a").two("b").exec(&mut db).await?;

    let empty: Vec<(String, String)> = vec![];
    let items: Vec<_> = Item::filter(toasty::stmt::in_list::<(String, String)>(
        (Item::fields().one(), Item::fields().two()),
        empty,
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}
