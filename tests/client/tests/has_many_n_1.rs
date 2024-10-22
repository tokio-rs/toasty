use tests_client::*;

async fn hello_world(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            todos: [Todo],
        }

        model Todo {
            #[key]
            #[auto]
            id: Id,

            #[index]
            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            #[index]
            category_id: Id<Category>,

            #[relation(key = category_id, references = id)]
            category: Category,

            title: String,
        }

        model Category {
            #[key]
            #[auto]
            id: Id,

            todos: [Todo],

            name: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let cat1 = db::Category::create().name("a").exec(&db).await.unwrap();
    let cat2 = db::Category::create().name("b").exec(&db).await.unwrap();

    // Create a user with a few todos
    let user = db::User::create()
        .todo(db::Todo::create().category(&cat1).title("one"))
        .todo(db::Todo::create().category(&cat2).title("two"))
        .todo(db::Todo::create().category(&cat2).title("three"))
        .exec(&db)
        .await
        .unwrap();

    // Collect all categories
    // let mut cats = vec![];

    let _todos = user.todos().all(&db).await.unwrap();
}

tests!(hello_world,);
