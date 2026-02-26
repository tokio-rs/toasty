use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn clone_acquires_separate_connection(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
    }

    let mut db = t.setup_db(models!(Item)).await;

    // Before any operation, no connection has been acquired yet (the one from
    // push_schema during setup_db already holds one).
    let status = db.pool().status();
    assert_eq!(status.size, 1, "setup_db should have acquired one connection");
    assert_eq!(
        status.available, 0,
        "that connection should be in use by db"
    );

    // Clone the handle — the clone starts with no connection.
    let mut db2 = db.clone();

    // The clone hasn't done anything yet, pool state unchanged.
    let status = db.pool().status();
    assert_eq!(status.size, 1, "clone should not acquire a connection yet");

    // Use the clone — this should lazily acquire a second connection.
    Item::create().exec(&mut db2).await?;

    let status = db.pool().status();
    assert_eq!(
        status.size, 2,
        "using the clone should have acquired a second connection"
    );
    assert_eq!(
        status.available, 0,
        "both connections should be in use"
    );

    // Drop the clone — its connection should return to the pool.
    drop(db2);

    // The background task needs a moment to notice the channel closed and exit,
    // which drops the PoolConnection and returns it to the pool.
    tokio::task::yield_now().await;

    let status = db.pool().status();
    assert_eq!(status.size, 2, "pool should still have 2 connections total");
    assert_eq!(
        status.available, 1,
        "dropped clone's connection should be back in the pool"
    );

    // The original handle still works fine.
    let item = Item::create().exec(&mut db).await?;
    let found = Item::filter_by_id(item.id)
        .all(&mut db)
        .await?
        .collect::<Vec<_>>()
        .await?;
    assert_eq!(found.len(), 1);

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
