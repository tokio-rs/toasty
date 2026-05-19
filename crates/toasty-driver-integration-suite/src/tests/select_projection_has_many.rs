//! `.select(...)` projection through a `HasMany` relation field.
//!
//! Field handles for `HasMany` relations return
//! `<Target as Relation>::ManyField<__Origin>` (the macro-generated
//! `*FieldList` struct).  An `IntoExpr<List<TargetModel>>` impl on that
//! struct lets the field handle flow through `.select(...)` the same way
//! `BelongsTo`/`HasOne` handles do; each parent row projects to a list of
//! related rows, and the executor decodes the result as `Vec<Vec<Target>>`.

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_many_basic(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(User {
        name: "Alice",
        posts: [Post::create().title("alpha"), Post::create().title("beta"),],
    })
    .exec(&mut db)
    .await?;

    let posts_per_user: Vec<Vec<Post>> = User::all()
        .select(User::fields().posts())
        .exec(&mut db)
        .await?;

    assert_eq!(posts_per_user.len(), 1);
    let mut titles: Vec<String> = posts_per_user[0].iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_many_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(User {
        name: "Alice",
        posts: [Post::create().title("alpha")],
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        posts: [
            Post::create().title("beta one"),
            Post::create().title("beta two"),
        ],
    })
    .exec(&mut db)
    .await?;

    let posts_per_user: Vec<Vec<Post>> = User::filter(User::fields().name().eq("Bob"))
        .select(User::fields().posts())
        .exec(&mut db)
        .await?;

    assert_eq!(posts_per_user.len(), 1);
    let mut titles: Vec<String> = posts_per_user[0].iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["beta one".to_string(), "beta two".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn select_has_many_first(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(User {
        name: "Alice",
        posts: [Post::create().title("alpha"), Post::create().title("beta"),],
    })
    .exec(&mut db)
    .await?;

    let posts: Option<Vec<Post>> = User::filter(User::fields().name().eq("Alice"))
        .select(User::fields().posts())
        .first()
        .exec(&mut db)
        .await?;

    let posts = posts.expect("first() returned None for a matching user");
    let mut titles: Vec<String> = posts.iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}
