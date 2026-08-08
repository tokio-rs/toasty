//! Mutations whose filter is only resolved at runtime.
//!
//! On a key-value driver an UPDATE/DELETE runs in two steps: collect the keys
//! of the matching rows, then mutate each key with the residual filter attached
//! as a condition. When the filter contains an `in_query` the driver cannot
//! evaluate, the engine plans the subquery separately and leaves an `Arg` in
//! its place. Both steps carry their own copy of that filter, so both need the
//! argument substituted before the expression reaches the driver.

use crate::prelude::*;

/// DELETE on the scan path — no index covers `user_id`, so the keys come from
/// a full-table scan.
#[driver_test]
pub async fn scan_delete_with_subquery_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(Post::[
        { title: "alice's", user: { name: "alice" } },
        { title: "bob's", user: { name: "bob" } },
    ])
    .exec(&mut db)
    .await?;

    Post::filter(
        Post::fields()
            .user()
            .in_query(User::filter(User::fields().name().eq("alice"))),
    )
    .delete()
    .exec(&mut db)
    .await?;

    let remaining: Vec<Post> = Post::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("bob's", remaining[0].title);

    Ok(())
}

/// UPDATE on the scan path, the mutation counterpart to
/// [`scan_delete_with_subquery_filter`].
#[driver_test]
pub async fn scan_update_with_subquery_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,

        title: String,

        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(Post::[
        { title: "alice's", user: { name: "alice" } },
        { title: "bob's", user: { name: "bob" } },
    ])
    .exec(&mut db)
    .await?;

    Post::filter(
        Post::fields()
            .user()
            .in_query(User::filter(User::fields().name().eq("alice"))),
    )
    .update()
    .title("updated")
    .exec(&mut db)
    .await?;

    let mut titles: Vec<String> = Post::all()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|post: Post| post.title)
        .collect();
    titles.sort();

    assert_eq!(vec!["bob's", "updated"], titles);

    Ok(())
}

/// DELETE keyed by a partition key with a runtime-dependent residual filter.
/// The keys come from a partition query rather than a scan, and the residual
/// filter is handed to that query as well as to the per-key delete.
#[driver_test]
pub async fn partition_delete_with_subquery_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = title)]
    struct Post {
        team: String,

        title: String,

        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(Post::[
        { team: "red", title: "alice's", user: { name: "alice" } },
        { team: "red", title: "bob's", user: { name: "bob" } },
    ])
    .exec(&mut db)
    .await?;

    Post::filter(
        Post::fields().team().eq("red").and(
            Post::fields()
                .user()
                .in_query(User::filter(User::fields().name().eq("alice"))),
        ),
    )
    .delete()
    .exec(&mut db)
    .await?;

    let remaining: Vec<Post> = Post::all().exec(&mut db).await?;

    assert_eq!(1, remaining.len());
    assert_eq!("bob's", remaining[0].title);

    Ok(())
}
