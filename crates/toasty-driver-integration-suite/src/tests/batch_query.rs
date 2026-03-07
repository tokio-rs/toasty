use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_models(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        #[index]
        title: String,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    Post::create().title("Hello").exec(&mut db).await?;

    let (users, posts): (Vec<User>, Vec<Post>) = toasty::batch((
        User::filter_by_name("Alice"),
        Post::filter_by_title("Hello"),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(users, [_ { name: "Alice" }]);
    assert_struct!(posts, [_ { title: "Hello" }]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn batch_one_empty(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        #[index]
        title: String,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    User::create().name("Alice").exec(&mut db).await?;

    let (users, posts): (Vec<User>, Vec<Post>) = toasty::batch((
        User::filter_by_name("Alice"),
        Post::filter_by_title("nonexistent"),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(users, [_ { name: "Alice" }]);
    assert!(posts.is_empty());

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn batch_same_model(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    User::create().name("Carol").exec(&mut db).await?;

    let (alices, bobs): (Vec<User>, Vec<User>) =
        toasty::batch((User::filter_by_name("Alice"), User::filter_by_name("Bob")))
            .exec(&mut db)
            .await?;

    assert_struct!(alices, [_ { name: "Alice" }]);
    assert_struct!(bobs, [_ { name: "Bob" }]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn batch_three_queries(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    User::create().name("Carol").exec(&mut db).await?;

    let (alices, bobs, carols): (Vec<User>, Vec<User>, Vec<User>) = toasty::batch((
        User::filter_by_name("Alice"),
        User::filter_by_name("Bob"),
        User::filter_by_name("Carol"),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(alices, [_ { name: "Alice" }]);
    assert_struct!(bobs, [_ { name: "Bob" }]);
    assert_struct!(carols, [_ { name: "Carol" }]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn batch_both_empty(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        #[index]
        title: String,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let (users, posts): (Vec<User>, Vec<Post>) = toasty::batch((
        User::filter_by_name("nobody"),
        Post::filter_by_title("nothing"),
    ))
    .exec(&mut db)
    .await?;

    assert!(users.is_empty());
    assert!(posts.is_empty());

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn batch_select_and_create(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create().name("Alice").exec(&mut db).await?;

    let (users, created): (Vec<User>, User) = toasty::batch((
        User::filter_by_name("Alice"),
        User::create().name("Bob"),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(users, [_ { name: "Alice" }]);
    assert_eq!(created.name, "Bob");

    // Verify Bob was actually persisted
    let bob = User::filter_by_name("Bob").first(&mut db).await?;
    assert!(bob.is_some());

    Ok(())
}
