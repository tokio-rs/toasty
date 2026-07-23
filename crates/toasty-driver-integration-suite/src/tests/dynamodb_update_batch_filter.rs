//! Tests for the non-unique multi-key update path on DynamoDB.
//!
//! When a compound filter is applied to an update (e.g. secondary-index column
//! AND non-indexed column), the planner routes it through `UpdateByKey` with
//! `keys` populated from a `FindPkByIndex` scan and `filter` set to the
//! non-indexed predicate.  The engine shreds the multi-key update into one
//! single-key `UpdateByKey` op per key, so each key's filter is adjudicated
//! independently — matching SQL's per-row semantics.  These updates are not
//! atomic.
//!
//! These tests verify the per-row semantics: every key matching → all updated,
//! no key matching → none updated, and a partial match → only the matching
//! subset is updated.

use crate::prelude::*;

#[driver_test(id(ID), requires(not(sql)), scenario(crate::scenarios::user_with_age))]
pub async fn update_via_secondary_index_uses_primary_key_type(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User {
        name: "Alice",
        age: 7,
    })
    .exec(&mut db)
    .await?;

    User::filter_by_age(7)
        .update()
        .name("updated")
        .exec(&mut db)
        .await?;

    let users = User::filter_by_age(7).exec(&mut db).await?;
    assert_struct!(users, [{ name: "updated", age: 7 }]);

    Ok(())
}

/// All items match the filter — every key is updated.
#[driver_test(requires(not(sql)), scenario(crate::scenarios::tagged_item))]
pub async fn batch_update_filter_all_match(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

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

/// A subset of items match the filter — only the matching keys are updated;
/// the rest are left untouched.  A single `transact_write_items` could not
/// express this (any condition failure cancels the whole transaction), so this
/// is the case that shredding fixes.
#[driver_test(requires(not(sql)), scenario(crate::scenarios::tagged_item))]
pub async fn batch_update_filter_partial_match(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(Item {
        tag: "batch",
        status: "active",
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
    toasty::create!(Item {
        tag: "batch",
        status: "active",
        name: "gamma"
    })
    .exec(&mut db)
    .await?;

    // alpha and gamma match (status == active); beta does not.
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
    assert_eq!(3, items.len());
    for item in &items {
        if item.status == "active" {
            assert_eq!(item.name, "updated");
        } else {
            assert_eq!(item.name, "beta");
        }
    }

    Ok(())
}

/// No items match the filter — every key's filter misses, so nothing is
/// updated and the call returns count 0.
#[driver_test(requires(not(sql)), scenario(crate::scenarios::tagged_item))]
pub async fn batch_update_filter_no_match(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

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
