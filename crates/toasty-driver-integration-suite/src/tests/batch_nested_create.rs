use crate::prelude::*;

/// Use a tuple of create builders to create multiple nested HasMany records
/// in a single parent create statement.
#[driver_test(id(ID))]
pub async fn batch_as_nested_has_many_create(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Pass a tuple of create builders directly — tuples implement
    // `IntoExpr<List<Model>>` so they work as nested HasMany values.
    let user = User::create()
        .name("Ann Chovey")
        .todos((
            Todo::create().title("Make pizza"),
            Todo::create().title("Sleep"),
        ))
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Ann Chovey");

    // Verify both todos were created and linked
    let todos: Vec<_> = user.todos().all(&mut db).await?;
    assert_eq_unordered!(todos.iter().map(|t| &t.title[..]), ["Make pizza", "Sleep"]);

    Ok(())
}
