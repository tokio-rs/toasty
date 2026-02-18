use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn different_field_name(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many(pair = owner)]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[belongs_to(key = owner_id, references = id)]
        owner: toasty::BelongsTo<User>,

        #[index]
        owner_id: ID,

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&db).await?;

    // Create a Todo associated with the user
    let todo = user.todos().create().title("hello world").exec(&db).await?;

    assert_eq!(todo.title, "hello world");

    // Load the user
    let user_reloaded = todo.owner().get(&db).await?;

    assert_eq!(user.id, user_reloaded.id);
    Ok(())
}
