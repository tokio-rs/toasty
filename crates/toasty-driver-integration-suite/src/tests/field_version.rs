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

/// Model with a secondary index used for multi-key update tests.
macro_rules! version_model_with_tag {
    () => {
        #[derive(Debug, toasty::Model)]
        struct Item {
            #[key]
            #[auto]
            id: uuid::Uuid,

            #[index]
            tag: String,

            name: String,

            #[version]
            version: u64,
        }
    };
}

/// Model with a unique field used for unique-index update tests.
macro_rules! version_model_with_unique {
    () => {
        #[derive(Debug, toasty::Model)]
        struct User {
            #[key]
            #[auto]
            id: uuid::Uuid,

            #[unique]
            email: String,

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

/// Query-based update on a versioned model: exercises update_by_key path 2
/// (no unique index, N keys via transact_write_items on DDB).
///
/// Query-based updates don't carry a per-item version condition, so the version
/// column is not incremented. The test verifies that the multi-key transact
/// path executes without error and applies all assignments.
#[driver_test(requires(not(sql)))]
pub async fn query_update_multi_key_works(test: &mut Test) -> Result<()> {
    version_model_with_tag!();

    let mut db = test.setup_db(models!(Item)).await;

    // Create two items sharing the same tag
    let a = Item::create()
        .tag("batch")
        .name("alpha")
        .exec(&mut db)
        .await?;
    let b = Item::create()
        .tag("batch")
        .name("beta")
        .exec(&mut db)
        .await?;

    // Update all items with tag == "batch" in one query-based operation.
    Item::filter_by_tag("batch")
        .update()
        .name("updated")
        .exec(&mut db)
        .await?;

    let a2 = Item::filter_by_id(a.id).get(&mut db).await?;
    let b2 = Item::filter_by_id(b.id).get(&mut db).await?;
    assert_eq!(a2.name, "updated");
    assert_eq!(b2.name, "updated");

    Ok(())
}

/// Updating a record through the unique-index path (path 3) increments the
/// version when the unique column changes.
#[driver_test(requires(not(sql)))]
pub async fn unique_index_update_increments_version(test: &mut Test) -> Result<()> {
    version_model_with_unique!();

    let mut db = test.setup_db(models!(User)).await;

    let mut user = User::create()
        .email("alice@example.com")
        .exec(&mut db)
        .await?;
    assert_eq!(user.version, 1);

    user.update()
        .email("alice2@example.com")
        .exec(&mut db)
        .await?;
    assert_eq!(user.version, 2);

    user.update()
        .email("alice3@example.com")
        .exec(&mut db)
        .await?;
    assert_eq!(user.version, 3);

    Ok(())
}

/// Stale update on a model with a unique index: the second update from a stale
/// snapshot should fail.
#[driver_test(requires(not(sql)))]
pub async fn unique_index_stale_update_fails(test: &mut Test) -> Result<()> {
    version_model_with_unique!();

    let mut db = test.setup_db(models!(User)).await;

    let mut user = User::create()
        .email("bob@example.com")
        .exec(&mut db)
        .await?;
    assert_eq!(user.version, 1);

    let mut stale = User::filter_by_email("bob@example.com")
        .get(&mut db)
        .await?;
    assert_eq!(stale.version, 1);

    // Advance user.version to 2
    user.update()
        .email("bob2@example.com")
        .exec(&mut db)
        .await?;
    assert_eq!(user.version, 2);

    // Stale handle (version == 1) should fail
    let result: Result<()> = stale.update().email("bob3@example.com").exec(&mut db).await;
    assert!(
        result.is_err(),
        "expected stale unique-index update to fail"
    );

    Ok(())
}

/// Deleting a record checks the version — a fresh handle succeeds.
#[driver_test(requires(not(sql)))]
pub async fn delete_checks_version(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let item = Item::create().name("hello").exec(&mut db).await?;
    assert_eq!(item.version, 1);
    let id = item.id;

    item.delete().exec(&mut db).await?;

    // Item should be gone — get() should return not-found
    let after_delete = Item::filter_by_id(id).get(&mut db).await;
    assert!(after_delete.is_err(), "item should have been deleted");

    Ok(())
}

/// Deleting from a stale snapshot (wrong version) should fail.
#[driver_test(requires(not(sql)))]
pub async fn stale_delete_fails(test: &mut Test) -> Result<()> {
    version_model!();

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = Item::create().name("hello").exec(&mut db).await?;
    assert_eq!(item.version, 1);

    // Load a stale copy and then advance item.version to 2.
    let stale = Item::filter_by_id(item.id).get(&mut db).await?;
    item.update().name("updated").exec(&mut db).await?;
    assert_eq!(item.version, 2);

    // stale.version == 1 — delete should fail.
    let result: Result<()> = stale.delete().exec(&mut db).await;
    assert!(result.is_err(), "expected stale delete to fail");

    // Item should still exist.
    let _ = Item::filter_by_id(item.id)
        .get(&mut db)
        .await
        .expect("item should still exist");

    Ok(())
}
