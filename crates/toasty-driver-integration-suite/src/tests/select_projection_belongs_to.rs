//! `.select(...)` projection through a `BelongsTo` relation field.
//!
//! Projects the related-model side of the relation directly: a query rooted
//! at the source model returns one related-model record per source row.

use crate::prelude::*;

#[driver_test(id(ID), requires(scan))]
pub async fn select_belongs_to_basic(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        author_id: ID,

        #[belongs_to(key = author_id, references = id)]
        author: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(Post {
        title: "Hello",
        author: alice
    })
    .exec(&mut db)
    .await?;

    let users: Vec<User> = Post::all()
        .select(Post::fields().author())
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn select_belongs_to_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        author_id: ID,

        #[belongs_to(key = author_id, references = id)]
        author: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;
    toasty::create!(Post::[
        { title: "Alpha", author: alice },
        { title: "Beta",  author: bob },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = Post::filter(Post::fields().title().eq("Beta"))
        .select(Post::fields().author())
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");
    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn select_belongs_to_first(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        author_id: ID,

        #[belongs_to(key = author_id, references = id)]
        author: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(Post {
        title: "Hello",
        author: alice
    })
    .exec(&mut db)
    .await?;

    let user: Option<User> = Post::filter(Post::fields().title().eq("Hello"))
        .select(Post::fields().author())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(user.map(|u| u.name).as_deref(), Some("Alice"));
    Ok(())
}
