//! Tests for the non-unique multi-key update path on DynamoDB.
//!
//! When a compound filter is applied to an update (e.g. secondary-index column
//! AND non-indexed column), the planner routes it through `UpdateByKey` with
//! `keys` populated from a `FindPkByIndex` scan and `filter` set to the
//! non-indexed predicate.  With multiple keys the driver uses
//! `transact_write_items`; if any item fails the filter condition DynamoDB
//! returns `TransactionCanceledException`.
//!
//! Before the fix the driver hit `todo!()` on that error.  These tests verify
//! the corrected semantics: filter miss → count 0 (no panic), no miss → all
//! items updated.

use crate::prelude::*;

macro_rules! tagged_item {
    () => {
        #[derive(Debug, toasty::Model)]
        struct Item {
            #[key]
            #[auto]
            id: uuid::Uuid,

            /// Indexed so the planner uses FindPkByIndex to collect keys.
            #[index]
            tag: String,

            /// Not indexed; becomes the `result_filter` on UpdateByKey.
            status: String,

            name: String,
        }
    };
}

/// All items match the filter — every key is updated.
#[driver_test(requires(not(sql)))]
pub async fn batch_update_filter_all_match(t: &mut Test) -> Result<()> {
    tagged_item!();

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item {
        tag: "batch",
        status: "active",
        name: "alpha"
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Item {
        tag: "batch",
        status: "active",
        name: "beta"
    })
    .exec(&mut db)
    .await?;

    Item::filter(
        Item::fields()
            .tag()
            .eq("batch")
            .and(Item::fields().status().eq("active")),
    )
    .update()
    .name("updated")
    .exec(&mut db)
    .await?;

    let items: Vec<Item> = Item::filter_by_tag("batch").exec(&mut db).await?;
    assert_eq!(2, items.len());
    assert!(items.iter().all(|i| i.name == "updated"));

    Ok(())
}

/// No items match the filter — the transact_write_items call is cancelled with
/// ConditionalCheckFailed for every item.  The driver must return count 0 rather
/// than panicking.
#[driver_test(requires(not(sql)))]
pub async fn batch_update_filter_no_match(t: &mut Test) -> Result<()> {
    tagged_item!();

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item {
        tag: "batch",
        status: "inactive",
        name: "alpha"
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Item {
        tag: "batch",
        status: "inactive",
        name: "beta"
    })
    .exec(&mut db)
    .await?;

    // This must not panic even though both items fail the status filter.
    Item::filter(
        Item::fields()
            .tag()
            .eq("batch")
            .and(Item::fields().status().eq("active")),
    )
    .update()
    .name("updated")
    .exec(&mut db)
    .await?;

    // Items are unchanged.
    let items: Vec<Item> = Item::filter_by_tag("batch").exec(&mut db).await?;
    assert_eq!(2, items.len());
    assert!(items.iter().all(|i| i.name != "updated"));

    Ok(())
}
