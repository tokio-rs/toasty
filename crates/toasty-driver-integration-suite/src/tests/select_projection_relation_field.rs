//! Projecting a single primitive field through a relation builder.
//!
//! The codegen emits per-primitive-field methods on `Many<Kind>` and on the
//! `{Model}Query` struct that delegate to `.select(...)`. So
//! `user.todos().title().exec(&db)` runs a query that projects only the
//! `title` column instead of round-tripping the whole `Todo` row.

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn many_field_projection_basic(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;

    let user = toasty::create!(User {
        name: "Alice",
        todos: [Todo::create().title("alpha"), Todo::create().title("beta"),],
    })
    .exec(&mut db)
    .await?;

    let mut titles: Vec<String> = user.todos().title().exec(&mut db).await?;
    titles.sort();
    assert_eq!(titles, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn many_field_projection_after_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;

    let user = toasty::create!(User {
        name: "Alice",
        todos: [
            Todo::create().title("alpha"),
            Todo::create().title("beta"),
            Todo::create().title("gamma"),
        ],
    })
    .exec(&mut db)
    .await?;

    let titles: Vec<String> = user
        .todos()
        .filter(Todo::fields().title().eq("beta"))
        .title()
        .exec(&mut db)
        .await?;
    assert_eq!(titles, vec!["beta".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn query_struct_field_projection(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item { name: "alpha" })
        .exec(&mut db)
        .await?;
    toasty::create!(Item { name: "beta" }).exec(&mut db).await?;

    let mut names: Vec<String> = Item::all().name().exec(&mut db).await?;
    names.sort();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}
