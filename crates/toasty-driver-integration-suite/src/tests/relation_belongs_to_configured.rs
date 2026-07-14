use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn different_field_name(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many(pair = owner)]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[belongs_to(key = owner_id, references = id)]
        owner: toasty::Deferred<User>,

        #[index]
        owner_id: ID,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&mut db).await?;

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&mut db)
        .await?;

    assert_eq!(todo.title, "hello world");

    // Load the user
    let user_reloaded = todo.owner().exec(&mut db).await?;

    assert_eq!(user.id, user_reloaded.id);
    Ok(())
}

// Regression test for https://github.com/tokio-rs/toasty/issues/924:
// `#[has_one(pair = <field>)]` was accepted but ignored, so the generated
// back-reference check still looked for a `BelongsTo` field named after the
// parent model instead of the configured pair.
#[driver_test(id(ID))]
pub async fn has_one_different_field_name(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,

        #[has_one(pair = owner)]
        other: toasty::Deferred<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: ID,

        #[belongs_to(key = owner_id, references = id)]
        owner: toasty::Deferred<Parent>,

        #[unique]
        owner_id: ID,
    }

    let mut db = test.setup_db(models!(Parent, Child)).await;

    let parent = Parent::create()
        .other(Child::create())
        .exec(&mut db)
        .await?;

    let child = parent.other().exec(&mut db).await?;
    assert_eq!(child.owner_id, parent.id);

    let parent_reloaded = child.owner().exec(&mut db).await?;
    assert_eq!(parent.id, parent_reloaded.id);

    Ok(())
}
