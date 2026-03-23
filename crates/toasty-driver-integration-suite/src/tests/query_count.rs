use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn count_empty_table(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let count = Item::all().count().exec(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn count_after_inserts(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    Item::create().name("a").exec(&mut db).await?;
    Item::create().name("b").exec(&mut db).await?;
    Item::create().name("c").exec(&mut db).await?;

    let count = Item::all().count().exec(&mut db).await?;
    assert_eq!(count, 3);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn count_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    Item::create().name("a").exec(&mut db).await?;
    Item::create().name("a").exec(&mut db).await?;
    Item::create().name("b").exec(&mut db).await?;

    let count = Item::filter_by_name("a").count().exec(&mut db).await?;
    assert_eq!(count, 2);

    let count = Item::filter_by_name("b").count().exec(&mut db).await?;
    assert_eq!(count, 1);

    let count = Item::filter_by_name("c").count().exec(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}
