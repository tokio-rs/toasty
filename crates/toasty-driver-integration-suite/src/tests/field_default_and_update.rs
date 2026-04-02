use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn default_expr_on_create(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[default(5)]
        view_count: i64,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    // Create without setting view_count — should get the default
    let created = Item::create().title("hello").exec(&mut db).await?;
    assert_eq!(created.view_count, 5);

    // Read back from DB
    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.view_count, 5);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn default_expr_override(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[default(5)]
        view_count: i64,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    // Override the default by explicitly setting view_count
    let created = Item::create()
        .title("hello")
        .view_count(42)
        .exec(&mut db)
        .await?;
    assert_eq!(created.view_count, 42);

    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.view_count, 42);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_expr_on_create(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    let before = Timestamp::now();
    let created = Item::create().title("hello").exec(&mut db).await?;
    let after = Timestamp::now();

    // updated_at should be auto-populated on create
    assert!(created.updated_at >= before);
    assert!(created.updated_at <= after);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_expr_on_update(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    let mut item = Item::create().title("hello").exec(&mut db).await?;
    let created_ts = item.updated_at;

    // Small delay to ensure timestamp changes
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let before = Timestamp::now();
    item.update().title("updated").exec(&mut db).await?;
    let after = Timestamp::now();

    // updated_at should have been refreshed
    assert!(item.updated_at >= before);
    assert!(item.updated_at <= after);
    assert!(item.updated_at > created_ts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_expr_override_on_update(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    let mut item = Item::create().title("hello").exec(&mut db).await?;

    // Override the update expression with an explicit value
    let explicit_ts = Timestamp::from_second(946684800).unwrap(); // 2000-01-01
    item.update()
        .title("updated")
        .updated_at(explicit_ts)
        .exec(&mut db)
        .await?;

    assert_eq!(item.updated_at, explicit_ts);

    let read = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(read.updated_at, explicit_ts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn default_and_update_on_same_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        // On create: defaults to "draft". On update: automatically set to "edited".
        #[default("draft".to_string())]
        #[update("edited".to_string())]
        status: String,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    // On create, #[default] takes priority
    let mut item = Item::create().title("hello").exec(&mut db).await?;
    assert_eq!(item.status, "draft");

    // On update, #[update] applies
    item.update().title("updated").exec(&mut db).await?;
    assert_eq!(item.status, "edited");

    // Explicit override on create
    let mut item2 = Item::create()
        .title("hello")
        .status("published".to_string())
        .exec(&mut db)
        .await?;
    assert_eq!(item2.status, "published");

    // Explicit override on update
    item2
        .update()
        .title("updated")
        .status("archived".to_string())
        .exec(&mut db)
        .await?;
    assert_eq!(item2.status, "archived");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn auto_on_timestamp_fields(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[auto]
        created_at: Timestamp,

        #[auto]
        updated_at: Timestamp,
    }

    let mut db = test.setup_db(toasty::models!(Item)).await;

    let before = Timestamp::now();
    let mut item = Item::create().title("hello").exec(&mut db).await?;
    let after = Timestamp::now();

    assert!(item.created_at >= before);
    assert!(item.created_at <= after);
    assert!(item.updated_at >= before);
    assert!(item.updated_at <= after);

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let before_update = Timestamp::now();
    item.update().title("updated").exec(&mut db).await?;
    let after_update = Timestamp::now();

    // created_at stays the same, updated_at is refreshed
    assert!(item.created_at <= after);
    assert!(item.updated_at >= before_update);
    assert!(item.updated_at <= after_update);

    Ok(())
}
