//! An UPDATE whose filter carries a key that no longer resolves to a row is a
//! no-op, not an error.
//!
//! Key-value drivers resolve the key first and hand the remaining filter to the
//! driver as a write condition. When the row behind that key is gone — deleted
//! between key collection and the per-key mutation, or never present — the
//! update leaves it alone, the same outcome a SQL `UPDATE ... WHERE` gives. On
//! DynamoDB an update assigning a unique-indexed column takes a separate path
//! that reads the row before writing, so both shapes are covered.

use crate::prelude::*;

#[driver_test]
pub async fn update_by_key_missing_row_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item { category: "keep" })
        .exec(&mut db)
        .await?;

    Item::filter(
        Item::fields()
            .id()
            .eq(uuid::Uuid::new_v4())
            .and(Item::fields().category().eq("keep")),
    )
    .update()
    .category("changed")
    .exec(&mut db)
    .await?;

    let remaining: Vec<Item> = Item::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("keep", remaining[0].category);

    Ok(())
}

/// Assigning a unique-indexed column routes the DynamoDB update through a
/// preliminary read that synchronizes the unique-index table. A missing row
/// must fail the filter, not error before the filter is consulted.
#[driver_test]
pub async fn update_by_key_missing_row_with_filter_and_unique_index(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[unique]
        email: String,

        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item {
        email: "alice@example.com",
        category: "keep",
    })
    .exec(&mut db)
    .await?;

    Item::filter(
        Item::fields()
            .id()
            .eq(uuid::Uuid::new_v4())
            .and(Item::fields().category().eq("keep")),
    )
    .update()
    .email("bob@example.com")
    .exec(&mut db)
    .await?;

    let remaining: Vec<Item> = Item::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("alice@example.com", remaining[0].email);

    Ok(())
}
