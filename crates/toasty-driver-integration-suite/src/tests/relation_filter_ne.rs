use crate::prelude::*;

/// Tests `ne` on a `belongs_to` relation handle: the dual of relation
/// `eq`, lowering to a comparison on the foreign key.
#[driver_test(requires(scan))]
pub async fn belongs_to_ne_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many]
        todos: toasty::Deferred<Vec<Todo>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
        #[index]
        user_id: uuid::Uuid,
        #[belongs_to]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Todo)).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;
    toasty::create!(Todo {
        title: "a",
        user_id: alice.id,
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Todo {
        title: "b",
        user_id: bob.id,
    })
    .exec(&mut db)
    .await?;

    let todos = Todo::filter(Todo::fields().user().ne(&alice))
        .exec(&mut db)
        .await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "b");

    Ok(())
}
