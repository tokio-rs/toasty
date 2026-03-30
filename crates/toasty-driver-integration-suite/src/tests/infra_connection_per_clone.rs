use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn db_does_not_hold_connection(t: &mut Test) -> Result<()> {
    if !t.capability().test_connection_pool {
        return Ok(());
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Db is stateless — after setup_db, the connection used for push_schema has
    // been returned to the pool, so it should be available.
    let status = db.pool().status();
    assert_eq!(
        status.size, 1,
        "setup_db should have created one connection"
    );
    assert_eq!(
        status.available, 1,
        "connection should be back in the pool (Db is stateless)"
    );

    // Execute an operation — it acquires a connection, runs, and returns it.
    Item::create().exec(&mut db).await?;

    let status = db.pool().status();
    assert_eq!(
        status.available, 1,
        "connection should be returned after operation"
    );

    // Clone the handle — both share the same pool.
    let mut db2 = db.clone();
    Item::create().exec(&mut db2).await?;

    let status = db.pool().status();
    assert_eq!(
        status.available, status.size,
        "all connections should be available after operations"
    );

    // The original handle still works fine.
    let item = Item::create().exec(&mut db).await?;
    let found = Item::filter_by_id(item.id).exec(&mut db).await?;
    assert_eq!(found.len(), 1);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn dedicated_connection_holds_pool_slot(t: &mut Test) -> Result<()> {
    if !t.capability().test_connection_pool {
        return Ok(());
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
    }

    let db = t.setup_db(models!(Item)).await;

    // All connections available since Db is stateless.
    let status = db.pool().status();
    assert_eq!(status.available, status.size);

    // Acquire a dedicated connection — it should be held from the pool.
    let mut conn = db.connection().await?;

    let status = db.pool().status();
    assert_eq!(
        status.available,
        status.size - 1,
        "dedicated connection should be held from the pool"
    );

    // Use the connection.
    Item::create().exec(&mut conn).await?;

    // Connection is still held.
    let status = db.pool().status();
    assert_eq!(
        status.available,
        status.size - 1,
        "connection should still be held after operation"
    );

    // Drop the connection — it should return to the pool.
    drop(conn);

    // The background task needs a moment to notice the channel closed and exit.
    tokio::task::yield_now().await;

    let status = db.pool().status();
    assert_eq!(
        status.available, status.size,
        "connection should be returned after drop"
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn write_visible_on_same_handle(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Note {
        #[key]
        #[auto]
        id: ID,

        body: String,
    }

    let mut db = t.setup_db(models!(Note)).await;

    // Write and immediately read on the same handle — must see the write.
    let created = Note::create().body("hello").exec(&mut db).await?;
    let found = Note::filter_by_id(created.id).get(&mut db).await?;
    assert_eq!(found.body, "hello");

    Ok(())
}
