//! forum-relationships: how a forum loads and traverses related data.
//!
//! Run it cold (`cargo run -p example-forum-relationships`). The theme is one rule:
//! in Toasty `.await` runs a query and `.get()` reads already-loaded data — so an
//! N+1 query storm is visible right in the source, and easy to avoid. Along the way
//! it covers both directions of a relationship, a one-to-one profile, preloading
//! with `.include()`, a multi-step `via` relation, association filters, scoped
//! relation queries, and editing relationship membership.
//!
//! Uses in-memory SQLite by default; set `TOASTY_CONNECTION_URL` for another SQL
//! backend. Association filters and `via` relations compile to subqueries, so they
//! need a SQL backend (they are rejected on DynamoDB).

use toasty::stmt;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,

    // One-to-one (see `Profile`). `Deferred<Option<_>>`: a user may have no profile.
    #[has_one]
    profile: toasty::Deferred<Option<Profile>>,

    // One-to-many: the comments this user wrote.
    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,

    // A multi-step (`via`) relation: the distinct threads this user took part in,
    // reached by walking `comments -> thread`. It owns no column of its own, is
    // read-only, and de-duplicates targets.
    #[has_many(via = comments.thread)]
    participated_threads: toasty::Deferred<Vec<Thread>>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
    bio: String,

    // `#[unique]` (not `#[index]`) on the foreign key is what makes this one-to-one:
    // at most one profile may point at a given user. The FK is `Option` so a profile
    // can be detached without being deleted.
    #[unique]
    user_id: Option<uuid::Uuid>,
    #[belongs_to]
    user: toasty::Deferred<Option<User>>,
}

#[derive(Debug, toasty::Model)]
struct Thread {
    #[key]
    #[auto]
    id: uuid::Uuid,
    title: String,
    #[has_many]
    comments: toasty::Deferred<Vec<Comment>>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    #[auto]
    id: uuid::Uuid,
    body: String,

    // A comment belongs to BOTH a thread and a user — two `belongs_to` edges, each
    // backed by its own indexed foreign key.
    #[index]
    thread_id: uuid::Uuid,
    #[belongs_to]
    thread: toasty::Deferred<Thread>,

    #[index]
    user_id: uuid::Uuid,
    #[belongs_to]
    user: toasty::Deferred<User>,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let url =
        std::env::var("TOASTY_CONNECTION_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let mut db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect(&url)
        .await?;
    db.push_schema().await?;

    // --- one-to-one, and walking a relationship both ways ---------------------------
    // A fresh user has no profile yet: the accessor returns `None`, not an error.
    let mut alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    assert!(alice.profile().exec(&mut db).await?.is_none());

    // Attach one through an update; the unique foreign key is filled in for us.
    // (Assigning a has_one is one of the few things `update!` can't express, so this
    // uses the builder form.)
    alice
        .update()
        .profile(Profile::create().bio("Rustacean"))
        .exec(&mut db)
        .await?;
    let profile = alice
        .profile()
        .exec(&mut db)
        .await?
        .expect("has a profile now");
    println!("Alice's profile bio: {:?}", profile.bio);

    // Relationships traverse in both directions: from the child back up to the parent.
    let owner = profile
        .user()
        .exec(&mut db)
        .await?
        .expect("profile has a user");
    println!("...and it belongs back to {:?}", owner.name);

    // A second user, and two threads to talk in.
    let mut bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;
    let threads = toasty::create!(Thread::[
        { title: "Async Rust" },
        { title: "ORM design" },
    ])
    .exec(&mut db)
    .await?;
    let (async_thread, orm_thread) = (&threads[0], &threads[1]);

    // A comment links a user and a thread. Pass the parents by reference and Toasty
    // pulls out their ids to set both foreign keys.
    toasty::create!(Comment::[
        { body: "tokio is great", user: &alice, thread: async_thread },
        { body: "what about io_uring?", user: &bob, thread: async_thread },
        { body: "love the derive macro", user: &alice, thread: orm_thread },
    ])
    .exec(&mut db)
    .await?;

    // --- preloading: defeat N+1 with .include() ------------------------------------
    // Looping and calling `user.comments().exec()` per row would be one query PER
    // user. `.include()` folds the children into the SAME query; `.get()` then reads
    // them from memory — no `.await`, no extra round-trips.
    let users = User::all()
        .include(User::fields().comments())
        .exec(&mut db)
        .await?;
    for user in &users {
        println!(
            "{} wrote {} comment(s)",
            user.name,
            user.comments.get().len()
        );
    }

    // --- a multi-step `via` relation -----------------------------------------------
    // Walks `alice -> comments -> thread` and returns DISTINCT threads, even though
    // she commented in two different threads.
    let participated = alice.participated_threads().exec(&mut db).await?;
    println!("Alice participated in {} thread(s)", participated.len());

    // --- association filter: query parents by a fact about their children ----------
    // `.any(..)` becomes a subquery: users who have at least one comment in the
    // async thread.
    let in_async = User::filter(
        User::fields()
            .comments()
            .any(Comment::fields().thread_id().eq(async_thread.id)),
    )
    .exec(&mut db)
    .await?;
    println!("{} user(s) posted in Async Rust", in_async.len());

    // --- a relation accessor is itself a refinable query ---------------------------
    // Scope down to just Bob's comments within the async thread.
    let bobs_async = async_thread
        .comments()
        .filter(Comment::fields().user_id().eq(bob.id))
        .exec(&mut db)
        .await?;
    println!("Bob has {} comment(s) in Async Rust", bobs_async.len());

    // --- edit relationship membership ----------------------------------------------
    // A `{ ... }` in a has_many update creates and attaches a new child; its `user`
    // foreign key is set for us.
    toasty::update!(bob {
        comments: [{ body: "ports welcome", thread: orm_thread }],
    })
    .exec(&mut db)
    .await?;
    let adopted = bob.comments().exec(&mut db).await?;
    println!(
        "Bob now has {} comment(s) after attaching one",
        adopted.len()
    );
    // Detach the one we just added with `stmt::remove` (the builder form takes it
    // directly). Because `Comment.user_id` is required, detaching removes the comment
    // row rather than orphaning it with a null author.
    bob.update()
        .comments(stmt::remove(adopted.last().unwrap()))
        .exec(&mut db)
        .await?;

    println!("\ndone — both directions, preloading, via, and association filters in one pass.");
    Ok(())
}
