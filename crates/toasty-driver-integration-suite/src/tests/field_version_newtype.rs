use crate::prelude::*;

/// A `#[version]` field may be a tuple-newtype around `u64` rather than a bare
/// `u64`. These tests mirror the core scenarios in `field_version.rs` but use
/// a `Version(u64)` newtype embed to verify that the macro accepts the type and
/// the generated OCC logic still operates correctly.
///
/// DynamoDB only for now (`requires(not(sql))`) because the SQL drivers do not
/// yet implement the version-counter OCC path.

#[derive(Debug, Clone, Copy, PartialEq, toasty::Embed)]
struct Version(u64);

/// A newly created record starts with version == Version(1).
#[driver_test(requires(not(sql)))]
pub async fn newtype_version_create_initializes(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[version]
        version: Version,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let item = toasty::create!(Item { name: "hello" })
        .exec(&mut db)
        .await?;
    assert_eq!(item.version, Version(1));

    Ok(())
}

/// Updating a record through the newtype version field increments the counter.
#[driver_test(requires(not(sql)))]
pub async fn newtype_version_update_increments(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[version]
        version: Version,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item { name: "hello" })
        .exec(&mut db)
        .await?;
    assert_eq!(item.version, Version(1));

    item.update().name("world").exec(&mut db).await?;
    assert_eq!(item.version, Version(2));

    item.update().name("again").exec(&mut db).await?;
    assert_eq!(item.version, Version(3));

    Ok(())
}

/// A stale update (wrong version) must be rejected even when the version field
/// is a newtype.
#[driver_test(requires(not(sql)))]
pub async fn newtype_version_stale_update_fails(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[version]
        version: Version,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item { name: "hello" })
        .exec(&mut db)
        .await?;
    assert_eq!(item.version, Version(1));

    let mut stale = Item::filter_by_id(item.id).get(&mut db).await?;
    assert_eq!(stale.version, Version(1));

    item.update().name("updated").exec(&mut db).await?;
    assert_eq!(item.version, Version(2));

    let result: Result<()> = stale.update().name("should fail").exec(&mut db).await;
    assert!(result.is_err(), "expected stale update to fail");

    Ok(())
}

/// A stale delete (wrong version) must be rejected when the version field is a
/// newtype.
#[driver_test(requires(not(sql)))]
pub async fn newtype_version_stale_delete_fails(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,

        #[version]
        version: Version,
    }

    let mut db = t.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item { name: "hello" })
        .exec(&mut db)
        .await?;
    let stale = Item::filter_by_id(item.id).get(&mut db).await?;

    item.update().name("updated").exec(&mut db).await?;
    assert_eq!(item.version, Version(2));

    let result: Result<()> = stale.delete().exec(&mut db).await;
    assert!(result.is_err(), "expected stale delete to fail");

    Ok(())
}
