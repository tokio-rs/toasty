//! Test that generated query, create, and update structs implement Clone.

use crate::prelude::*;

/// Clone a filtered query and apply different modifiers to each copy.
#[driver_test(id(ID), requires(sql))]
pub async fn clone_query_with_different_modifiers(t: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,
    }

    let mut db = t.setup_db(toasty::models!(Item)).await;

    for _ in 0..10 {
        Item::create().name("a").exec(&mut db).await?;
    }
    Item::create().name("b").exec(&mut db).await?;

    let query = Item::filter_by_name("a");
    let limited: Vec<_> = query.clone().limit(3).exec(&mut db).await?;
    let all: Vec<_> = query.exec(&mut db).await?;

    assert_eq!(limited.len(), 3);
    assert_eq!(all.len(), 10);

    Ok(())
}

/// Clone a create builder, then override a field on the second copy.
#[driver_test(id(ID))]
pub async fn clone_create_builder(t: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = t.setup_db(toasty::models!(Item)).await;

    let builder = Item::create().name("original");
    let a = builder.clone().exec(&mut db).await?;
    let b = builder.name("overridden").exec(&mut db).await?;

    assert_eq!(a.name, "original");
    assert_eq!(b.name, "overridden");

    Ok(())
}

/// Clone a query-based update builder, then change the value on the second copy.
#[driver_test(id(ID), requires(sql))]
pub async fn clone_update_builder(t: &mut Test) -> Result<()> {
    #[derive(toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,
    }

    let mut db = t.setup_db(toasty::models!(Item)).await;

    let a = Item::create().name("a").exec(&mut db).await?;
    Item::create().name("b").exec(&mut db).await?;

    let update = Item::filter_by_id(a.id).update();
    update.clone().name("x").exec(&mut db).await?;

    let items: Vec<_> = Item::filter_by_name("x").exec(&mut db).await?;
    assert_eq!(items.len(), 1);

    update.name("y").exec(&mut db).await?;

    let items: Vec<_> = Item::filter_by_name("y").exec(&mut db).await?;
    assert_eq!(items.len(), 1);

    // "b" is untouched
    let items: Vec<_> = Item::filter_by_name("b").exec(&mut db).await?;
    assert_eq!(items.len(), 1);

    Ok(())
}
