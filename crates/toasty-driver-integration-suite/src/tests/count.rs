//! Test the `.count()` query method

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn count_empty_table(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let count = User::filter(User::fields().name().eq("nobody"))
        .count(&mut db)
        .await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn count_all(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    for name in ["Alice", "Bob", "Charlie"] {
        User::create().name(name).exec(&mut db).await?;
    }

    let count = User::all().count(&mut db).await?;
    assert_eq!(count, 3);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn count_with_indexed_filter(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 25), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&mut db).await?;
    }

    let count = User::filter_by_name("Alice").count(&mut db).await?;
    assert_eq!(count, 1);

    // Non-existent name
    let count = User::filter_by_name("Nobody").count(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn count_with_non_indexed_filter(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 25), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&mut db).await?;
    }

    let count = User::filter(User::fields().age().eq(25))
        .count(&mut db)
        .await?;
    assert_eq!(count, 2);

    let count = User::filter(User::fields().age().eq(99))
        .count(&mut db)
        .await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn count_after_insert_and_delete(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    assert_eq!(User::all().count(&mut db).await?, 0);

    let alice = User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    assert_eq!(User::all().count(&mut db).await?, 2);

    alice.delete().exec(&mut db).await?;
    assert_eq!(User::all().count(&mut db).await?, 1);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn count_after_insert_and_delete_indexed(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        group: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let count = User::filter_by_group("team-a").count(&mut db).await?;
    assert_eq!(count, 0);

    let alice = User::create().group("team-a").exec(&mut db).await?;
    User::create().group("team-a").exec(&mut db).await?;
    User::create().group("team-b").exec(&mut db).await?;

    let count = User::filter_by_group("team-a").count(&mut db).await?;
    assert_eq!(count, 2);

    alice.delete().exec(&mut db).await?;

    let count = User::filter_by_group("team-a").count(&mut db).await?;
    assert_eq!(count, 1);

    Ok(())
}
