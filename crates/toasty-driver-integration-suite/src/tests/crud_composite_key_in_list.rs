use crate::prelude::*;

#[driver_test]
pub async fn filter_composite_key_in_list(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        part_a: String,

        #[key]
        part_b: String,
    }

    let mut db = test.setup_db(models!(Item)).await;

    for i in 0..5 {
        Item::create()
            .part_a(format!("a-{i}"))
            .part_b(format!("b-{i}"))
            .exec(&mut db)
            .await?;
    }

    // Use the free function form with a tuple of field paths
    let items: Vec<_> = Item::filter(toasty::stmt::in_list::<(String, String)>(
        (Item::fields().part_a(), Item::fields().part_b()),
        [("a-1", "b-1"), ("a-3", "b-3")],
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 2);

    for item in &items {
        assert!(
            (item.part_a == "a-1" && item.part_b == "b-1")
                || (item.part_a == "a-3" && item.part_b == "b-3")
        );
    }

    Ok(())
}

#[driver_test]
pub async fn filter_composite_key_in_list_empty(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        part_a: String,

        #[key]
        part_b: String,
    }

    let mut db = test.setup_db(models!(Item)).await;

    Item::create().part_a("a").part_b("b").exec(&mut db).await?;

    let empty: Vec<(String, String)> = vec![];
    let items: Vec<_> = Item::filter(toasty::stmt::in_list::<(String, String)>(
        (Item::fields().part_a(), Item::fields().part_b()),
        empty,
    ))
    .exec(&mut db)
    .await?;

    assert_eq!(items.len(), 0);

    Ok(())
}
