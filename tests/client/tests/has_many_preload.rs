use tests_client::*;

async fn basic_has_many_and_belongs_to_preload(s: impl Setup) {
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

            title: String,
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with a few todos
    let user = db::User::create()
        .todo(db::Todo::create().title("one"))
        .todo(db::Todo::create().title("two"))
        .todo(db::Todo::create().title("three"))
        .exec(&db)
        .await
        .unwrap();

    // Find the user, include TODOs
    let user = db::User::filter_by_id(&user.id)
        .include(db::User::TODOS)
        .get(&db)
        .await
        .unwrap();

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id.clone();

    let todo = db::Todo::filter_by_id(&id)
        .include(db::Todo::USER)
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, todo.user.get().id);
}

tests!(basic_has_many_and_belongs_to_preload,);
