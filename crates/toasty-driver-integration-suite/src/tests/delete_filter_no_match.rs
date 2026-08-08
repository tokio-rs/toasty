//! A DELETE whose key matches but whose filter does not is a no-op, not an
//! error.
//!
//! Key-value drivers resolve the key first and hand the remaining filter to the
//! driver as a write condition. When that condition fails the row simply stays
//! put — the same outcome a SQL `DELETE ... WHERE` gives. On DynamoDB a model
//! with a unique index deletes through a transaction, which reports the miss
//! differently from a plain conditional delete, so both shapes are covered.

use crate::prelude::*;

#[driver_test]
pub async fn delete_by_key_with_unmatched_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        category: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item { category: "keep" })
        .exec(&mut db)
        .await?;

    Item::filter(
        Item::fields()
            .id()
            .eq(item.id)
            .and(Item::fields().category().eq("drop")),
    )
    .delete()
    .exec(&mut db)
    .await?;

    let remaining: Vec<Item> = Item::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("keep", remaining[0].category);

    Ok(())
}

/// The unique index sends the delete through a DynamoDB transaction, which
/// surfaces a filter miss as a cancelled transaction rather than a failed
/// conditional write.
#[driver_test]
pub async fn delete_by_key_with_unmatched_filter_and_unique_index(t: &mut Test) -> Result<()> {
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

    let item = toasty::create!(Item {
        email: "alice@example.com",
        category: "keep",
    })
    .exec(&mut db)
    .await?;

    Item::filter(
        Item::fields()
            .id()
            .eq(item.id)
            .and(Item::fields().category().eq("drop")),
    )
    .delete()
    .exec(&mut db)
    .await?;

    let remaining: Vec<Item> = Item::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("keep", remaining[0].category);

    Ok(())
}
