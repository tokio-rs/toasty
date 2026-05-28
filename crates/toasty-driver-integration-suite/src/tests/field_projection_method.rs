//! Generated `{field}()` projection methods on Query / Many / One.
//!
//! For every non-relation field on a model, the macro emits a method on the
//! model's `Query` and on each relation wrapper (`Many`, `One`, `OptionOne`)
//! that projects to that field's value type. `Deferred<T>` is stripped from
//! the return type via `Field::ExprTarget`.

use crate::prelude::*;

/// `Model::all().scalar_field()` projects to `Vec<T>` of the field's value.
#[driver_test(id(ID))]
pub async fn query_projects_scalar_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "alpha" },
        { name: "beta"  },
        { name: "gamma" },
    ])
    .exec(&mut db)
    .await?;

    let mut names: Vec<String> = Item::all().name().exec(&mut db).await?;
    names.sort();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);

    Ok(())
}

/// `.filter(...)` composes with the projection — only matching rows project.
#[driver_test(id(ID))]
pub async fn query_projects_with_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
        quantity: i64,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "a", quantity: 1_i64 },
        { name: "b", quantity: 5_i64 },
        { name: "c", quantity: 10_i64 },
    ])
    .exec(&mut db)
    .await?;

    let mut names: Vec<String> = Item::all()
        .filter(Item::fields().quantity().gt(2_i64))
        .name()
        .exec(&mut db)
        .await?;
    names.sort();
    assert_eq!(names, vec!["b", "c"]);

    Ok(())
}

/// A `Deferred<T>` field projects as `T`, not `Deferred<T>`.
#[driver_test(id(ID), requires(sql))]
pub async fn query_projects_strips_deferred(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
        body: toasty::Deferred<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item::[
        { name: "a", body: "alpha body" },
        { name: "b", body: "beta body"  },
    ])
    .exec(&mut db)
    .await?;

    let mut bodies: Vec<String> = Item::all().body().exec(&mut db).await?;
    bodies.sort();
    assert_eq!(bodies, vec!["alpha body", "beta body"]);

    Ok(())
}

/// `Model::all().many_relation().scalar_field()` projects across the relation.
#[driver_test(id(ID), requires(sql))]
pub async fn many_wrapper_projects_scalar_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
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
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(User {
        name: "Alice",
        posts: [Post::create().title("alpha"), Post::create().title("beta")],
    })
    .exec(&mut db)
    .await?;

    let mut titles: Vec<String> = User::all().posts().title().exec(&mut db).await?;
    titles.sort();
    assert_eq!(titles, vec!["alpha", "beta"]);

    Ok(())
}

/// `instance.has_many_relation().scalar_field()` on the `Many` wrapper
/// projects to `Vec<T>` of the field's value across the related rows.
#[driver_test(id(ID), requires(sql))]
pub async fn many_wrapper_instance_projects_scalar_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
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
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let user = toasty::create!(User {
        name: "Alice",
        posts: [Post::create().title("alpha"), Post::create().title("beta")],
    })
    .exec(&mut db)
    .await?;

    let mut titles: Vec<String> = user.posts().title().exec(&mut db).await?;
    titles.sort();
    assert_eq!(titles, vec!["alpha", "beta"]);

    Ok(())
}

/// `instance.belongs_to().scalar_field()` on a `One` wrapper executes as a
/// single value (not a `Vec`).
#[driver_test(id(ID), requires(sql))]
pub async fn one_wrapper_projects_scalar_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
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
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let post = toasty::create!(Post {
        title: "alpha",
        user: user,
    })
    .exec(&mut db)
    .await?;

    let author_name: String = post.user().name().exec(&mut db).await?;
    assert_eq!(author_name, "Alice");

    Ok(())
}

/// Field projections compose into `toasty::batch((..))` — each tuple element
/// is a `Query<List<T>>`, and the batch returns `(Vec<T1>, Vec<T2>, ...)`.
#[driver_test(id(ID), requires(scan))]
pub async fn batch_field_projections(t: &mut Test) -> Result<()> {
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
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob"   },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Post::[
        { title: "alpha" },
        { title: "beta"  },
    ])
    .exec(&mut db)
    .await?;

    let (mut user_names, mut post_titles): (Vec<String>, Vec<String>) =
        toasty::batch((User::all().name(), Post::all().title()))
            .exec(&mut db)
            .await?;

    user_names.sort();
    post_titles.sort();

    assert_eq!(user_names, vec!["Alice", "Bob"]);
    assert_eq!(post_titles, vec!["alpha", "beta"]);

    Ok(())
}

/// Field projections mix freely with full-model queries inside a batch.
#[driver_test(id(ID), requires(scan))]
pub async fn batch_field_projection_with_full_model(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(User)).await;

    toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob"   },
    ])
    .exec(&mut db)
    .await?;

    let (mut names, mut users): (Vec<String>, Vec<User>) =
        toasty::batch((User::all().name(), User::all()))
            .exec(&mut db)
            .await?;

    names.sort();
    users.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(names, vec!["Alice", "Bob"]);
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[1].name, "Bob");

    Ok(())
}
