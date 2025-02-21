use tests_client::*;

async fn different_field_name(s: impl Setup) {
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

            #[relation(key = owner_id, references = id)]
            owner: User,

            #[index]
            owner_id: Id<User>,

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user
    let user = db::User::create().exec(&db).await.unwrap();

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
