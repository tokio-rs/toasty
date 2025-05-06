use tests::*;
// use toasty::stmt::Id;

// What is this testing?
async fn hello_world(_s: impl Setup) {
    /*
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        #[index]
        category_id: Id<Category>,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,

        name: String,
    }

    let db = s.setup(models!(User, Todo, Category)).await;

    let cat1 = Category::create().name("a").exec(&db).await.unwrap();
    let cat2 = Category::create().name("b").exec(&db).await.unwrap();

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create().category(&cat1).title("one"))
        .todo(Todo::create().category(&cat2).title("two"))
        .todo(Todo::create().category(&cat2).title("three"))
        .exec(&db)
        .await
        .unwrap();

    // Collect all categories
    // let mut cats = vec![];

    let todos = user.todos().all(&db).await.unwrap();

    for todos in todos {
        println!("todo: {:?}", todos);
    }
    */
}

tests!(hello_world,);
