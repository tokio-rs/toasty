//! Test N+1 query behavior with has_many associations

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn hello_world(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

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

        #[index]
        category_id: ID,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        #[allow(dead_code)]
        todos: toasty::HasMany<Todo>,

        #[allow(dead_code)]
        name: String,
    }

    let db = test.setup_db(models!(User, Todo, Category)).await;

    let cat1 = Category::create().name("a").exec(&db).await?;
    let cat2 = Category::create().name("b").exec(&db).await?;

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create().category(&cat1).title("one"))
        .todo(Todo::create().category(&cat2).title("two"))
        .todo(Todo::create().category(&cat2).title("three"))
        .exec(&db)
        .await?;

    let todos = user.todos().all(&db).await?;

    let todos: Vec<_> = todos.collect().await?;
    assert_eq!(3, todos.len());
    Ok(())
}
