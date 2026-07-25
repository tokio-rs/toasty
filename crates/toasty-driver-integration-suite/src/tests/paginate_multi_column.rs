//! Test keyset pagination over a multi-column `order_by`.
//!
//! The cursor must be compared lexicographically: a row that ties the cursor
//! on a leading sort column but is beyond it on a later column belongs on the
//! next page. These are gated on `sql` because SQL drivers rewrite the cursor
//! into a WHERE filter; NoSQL drivers use a driver-level cursor instead.

use crate::prelude::*;
use toasty::stmt::Page;

#[driver_test(requires(sql))]
pub async fn paginate_multi_column_equal_leading_values(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    // All rows share the same leading sort value.
    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 1 },
    ])
    .exec(&mut db)
    .await?;

    let page: Page<Item> = Item::all()
        .order_by((Item::fields().group().asc(), Item::fields().id().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_struct!(page.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    let page: Page<Item> = page.next(&mut db).await?.unwrap();
    assert_struct!(page.items, [_ { id: 3, .. }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn paginate_multi_column_mixed_directions(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        id: i64,
        group: i64,
    }

    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { id: 1, group: 1 },
        { id: 2, group: 1 },
        { id: 3, group: 2 },
        { id: 4, group: 2 },
    ])
    .exec(&mut db)
    .await?;

    // group desc, id asc: 3, 4, 1, 2
    let page: Page<Item> = Item::all()
        .order_by((Item::fields().group().desc(), Item::fields().id().asc()))
        .paginate(2)
        .exec(&mut db)
        .await?;
    assert_struct!(page.items, [_ { id: 3, .. }, _ { id: 4, .. }]);

    let page: Page<Item> = page.next(&mut db).await?.unwrap();
    assert_struct!(page.items, [_ { id: 1, .. }, _ { id: 2, .. }]);

    Ok(())
}
