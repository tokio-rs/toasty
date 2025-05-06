use tests::*;
use toasty::stmt::Id;

async fn basic_has_many_and_belongs_to_preload(s: impl Setup) {
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
    }

    let db = s.setup(models!(User, Todo)).await;

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create())
        .todo(Todo::create())
        .todo(Todo::create())
        .exec(&db)
        .await
        .unwrap();

    // Find the user, include TODOs
    let user = User::filter_by_id(&user.id)
        .include(User::FIELDS.todos)
        .get(&db)
        .await
        .unwrap();

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id.clone();

    let todo = Todo::filter_by_id(&id)
        .include(Todo::FIELDS.user)
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
}

tests!(basic_has_many_and_belongs_to_preload,);
