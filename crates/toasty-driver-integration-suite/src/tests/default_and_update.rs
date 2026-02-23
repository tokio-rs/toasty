#![allow(clippy::disallowed_names)]

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn default_expr_on_create(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[default(5)]
        view_count: i64,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Create without setting view_count — should get the default
    let created = Foo::create().title("hello").exec(&db).await?;
    assert_eq!(created.view_count, 5);

    // Read back from DB
    let read = Foo::get_by_id(&db, &created.id).await?;
    assert_eq!(read.view_count, 5);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn default_expr_override(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[default(5)]
        view_count: i64,
    }

    let db = test.setup_db(models!(Foo)).await;

    // Override the default by explicitly setting view_count
    let created = Foo::create()
        .title("hello")
        .view_count(42)
        .exec(&db)
        .await?;
    assert_eq!(created.view_count, 42);

    let read = Foo::get_by_id(&db, &created.id).await?;
    assert_eq!(read.view_count, 42);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_expr_on_create(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let before = Timestamp::now();
    let created = Foo::create().title("hello").exec(&db).await?;
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
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let mut foo = Foo::create().title("hello").exec(&db).await?;
    let created_ts = foo.updated_at;

    // Small delay to ensure timestamp changes
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let before = Timestamp::now();
    foo.update().title("updated").exec(&db).await?;
    let after = Timestamp::now();

    // updated_at should have been refreshed
    assert!(foo.updated_at >= before);
    assert!(foo.updated_at <= after);
    assert!(foo.updated_at > created_ts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn update_expr_override_on_update(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[update(jiff::Timestamp::now())]
        updated_at: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let mut foo = Foo::create().title("hello").exec(&db).await?;

    // Override the update expression with an explicit value
    let explicit_ts = Timestamp::from_second(946684800).unwrap(); // 2000-01-01
    foo.update()
        .title("updated")
        .updated_at(explicit_ts)
        .exec(&db)
        .await?;

    assert_eq!(foo.updated_at, explicit_ts);

    let read = Foo::get_by_id(&db, &foo.id).await?;
    assert_eq!(read.updated_at, explicit_ts);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn default_and_update_on_same_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        // On create: defaults to "draft". On update: automatically set to "edited".
        #[default("draft".to_string())]
        #[update("edited".to_string())]
        status: String,
    }

    let db = test.setup_db(models!(Foo)).await;

    // On create, #[default] takes priority
    let mut foo = Foo::create().title("hello").exec(&db).await?;
    assert_eq!(foo.status, "draft");

    // On update, #[update] applies
    foo.update().title("updated").exec(&db).await?;
    assert_eq!(foo.status, "edited");

    // Explicit override on create
    let mut foo2 = Foo::create()
        .title("hello")
        .status("published".to_string())
        .exec(&db)
        .await?;
    assert_eq!(foo2.status, "published");

    // Explicit override on update
    foo2.update()
        .title("updated")
        .status("archived".to_string())
        .exec(&db)
        .await?;
    assert_eq!(foo2.status, "archived");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn auto_on_timestamp_fields(test: &mut Test) -> Result<()> {
    use jiff::Timestamp;

    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[auto]
        created_at: Timestamp,

        #[auto]
        updated_at: Timestamp,
    }

    let db = test.setup_db(models!(Foo)).await;

    let before = Timestamp::now();
    let mut foo = Foo::create().title("hello").exec(&db).await?;
    let after = Timestamp::now();

    assert!(foo.created_at >= before);
    assert!(foo.created_at <= after);
    assert!(foo.updated_at >= before);
    assert!(foo.updated_at <= after);

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let before_update = Timestamp::now();
    foo.update().title("updated").exec(&db).await?;
    let after_update = Timestamp::now();

    // created_at stays the same, updated_at is refreshed
    assert!(foo.created_at <= after);
    assert!(foo.updated_at >= before_update);
    assert!(foo.updated_at <= after_update);

    Ok(())
}
