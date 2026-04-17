use crate::prelude::*;

/// Model used by all version tests in this file.
macro_rules! version_model {
    () => {
        #[derive(Debug, toasty::Model)]
        struct Item {
            #[key]
            #[auto]
            id: uuid::Uuid,

            name: String,

            #[version]
            version: u64,
        }
    };
}

/// A newly created record starts with version == 1.
#[driver_test(requires(not(sql)))]
pub async fn create_initializes_version(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let item = Item::create().name("hello").exec(&mut db).await?;
    assert_eq!(item.version, 1);

    Ok(())
}

/// Updating a record increments the version.
#[driver_test(requires(not(sql)))]
pub async fn update_increments_version(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = Item::create().name("hello").exec(&mut db).await?;
    assert_eq!(item.version, 1);

    item.update().name("world").exec(&mut db).await?;
    assert_eq!(item.version, 2);

    item.update().name("again").exec(&mut db).await?;
    assert_eq!(item.version, 3);

    Ok(())
}

/// Two updates from the same stale snapshot — the second should fail with a
/// condition-check error because the DB version has already moved to 2.
#[driver_test(requires(not(sql)))]
pub async fn stale_update_fails(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = Item::create().name("hello").exec(&mut db).await?;
    assert_eq!(item.version, 1);

    // Load a second handle from the DB — same record, version == 1.
    let mut stale = Item::filter_by_id(item.id).get(&mut db).await?;
    assert_eq!(stale.version, 1);

    // First update succeeds: DB version goes 1 → 2.
    item.update().name("updated").exec(&mut db).await?;
    assert_eq!(item.version, 2);

    // Stale handle still has version == 1; this should fail.
    let result: Result<()> = stale.update().name("should fail").exec(&mut db).await;
    assert!(
        result.is_err(),
        "expected stale update to fail, but it succeeded"
    );

    Ok(())
}

/// Creating the same primary key twice should fail because of the
/// attribute_not_exists condition on the version column.
#[driver_test(requires(not(sql)))]
pub async fn duplicate_create_fails(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let item = Item::create().name("original").exec(&mut db).await?;

    let result = Item::create()
        .id(item.id)
        .name("duplicate")
        .exec(&mut db)
        .await;

    assert!(
        result.is_err(),
        "expected duplicate create to fail, but it succeeded"
    );

    Ok(())
}

/// Batch-creating multiple versioned items should initialize all versions to 1,
/// and a duplicate within the batch should fail.
#[driver_test(requires(not(sql)))]
pub async fn batch_insert_checks_version(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    // Create two items in a single batch — both should succeed with version == 1.
    let items = Item::create_many()
        .item(Item::create().name("first"))
        .item(Item::create().name("second"))
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.version == 1));

    // Attempt to batch-create a new item alongside a duplicate ID — should fail.
    let existing_id = items[0].id;
    let result = Item::create_many()
        .item(Item::create().id(existing_id).name("duplicate"))
        .item(Item::create().name("new"))
        .exec(&mut db)
        .await;

    assert!(
        result.is_err(),
        "expected batch create with duplicate to fail"
    );

    Ok(())
}
