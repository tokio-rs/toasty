use tests_client::*;

async fn different_field_name(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: toasty::stmt::Id<Self>,

        #[has_many]
        todos: [Todo],
    }

    #[derive(Debug)]
    #[toasty::model]
    struct Todo {
        #[key]
        #[auto]
        id: toasty::stmt::Id<Self>,

        #[belongs_to(key = owner_id, references = id)]
        owner: User,

        #[index]
        owner_id: toasty::stmt::Id<User>,

        title: String,
    }

    let db = s.setup(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&db).await.unwrap();

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Load the user
    let user_reloaded = todo.owner().get(&db).await.unwrap();

    assert_eq!(user.id, user_reloaded.id)
}

tests!(different_field_name,);
