use crate::prelude::*;

/// Batch two updates of the same model.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_updates_same_model(t: &mut Test) -> Result<()> {
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

    t.log().clear();
    let ((), ()): ((), ()) = toasty::batch((
        User::filter_by_name("Alice").update().name("Alice2"),
        User::filter_by_name("Bob").update().name("Bob2"),
    ))
    .exec(&mut db)
    .await?;

    // Verify updates applied
    let alice: Vec<User> = User::filter_by_name("Alice2").collect(&mut db).await?;
    assert_eq!(alice.len(), 1);
    let bob: Vec<User> = User::filter_by_name("Bob2").collect(&mut db).await?;
    assert_eq!(bob.len(), 1);

    Ok(())
}

/// Batch two deletes of the same model.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_two_deletes_same_model(t: &mut Test) -> Result<()> {
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

    t.log().clear();
    let ((), ()): ((), ()) = toasty::batch((
        User::filter_by_name("Alice").delete(),
        User::filter_by_name("Bob").delete(),
    ))
    .exec(&mut db)
    .await?;

    // Verify deletes applied, Carol remains
    let all: Vec<User> = User::all().collect(&mut db).await?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Carol");

    Ok(())
}

/// Batch mixing update and delete of different models.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_update_and_delete(t: &mut Test) -> Result<()> {
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
    Post::create().title("Hello").exec(&mut db).await?;

    t.log().clear();
    let ((), ()): ((), ()) = toasty::batch((
        User::filter_by_name("Alice").update().name("Alice2"),
        Post::filter_by_title("Hello").delete(),
    ))
    .exec(&mut db)
    .await?;

    // User updated
    let users: Vec<User> = User::filter_by_name("Alice2").collect(&mut db).await?;
    assert_eq!(users.len(), 1);

    // Post deleted
    let posts: Vec<Post> = Post::all().collect(&mut db).await?;
    assert_eq!(posts.len(), 0);

    Ok(())
}

/// Batch all four statement types: query, create, update, delete.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_all_four_statement_types(t: &mut Test) -> Result<()> {
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

    t.log().clear();
    let (queried, created, (), ()): (Vec<User>, User, (), ()) = toasty::batch((
        User::filter_by_name("Alice"),
        User::create().name("Carol"),
        User::filter_by_name("Alice").update().name("Alice2"),
        User::filter_by_name("Bob").delete(),
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(queried, [_ { name: "Alice" }]);
    assert_eq!(created.name, "Carol");

    // Verify update applied
    let alice: Vec<User> = User::filter_by_name("Alice2").collect(&mut db).await?;
    assert_eq!(alice.len(), 1);

    // Verify delete applied
    let bob: Vec<User> = User::filter_by_name("Bob").collect(&mut db).await?;
    assert_eq!(bob.len(), 0);

    // Carol was created
    let carol: Vec<User> = User::filter_by_name("Carol").collect(&mut db).await?;
    assert_eq!(carol.len(), 1);

    Ok(())
}

/// Batch a delete using the model instance builder.
#[driver_test(id(ID), requires(sql))]
pub async fn batch_instance_delete(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;
    let alice = User::create().name("Alice").exec(&mut db).await?;
    let bob = User::create().name("Bob").exec(&mut db).await?;

    t.log().clear();
    let ((), ()): ((), ()) = toasty::batch((alice.delete(), bob.delete()))
        .exec(&mut db)
        .await?;

    let all: Vec<User> = User::all().collect(&mut db).await?;
    assert_eq!(all.len(), 0);

    Ok(())
}
