use tests_client::*;

async fn remove_add_single_relation_option_belongs_to(s: impl Setup) {
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
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with some todos
    let user = db::User::create()
        .todo(db::Todo::create())
        .todo(db::Todo::create())
        .exec(&db)
        .await
        .unwrap();

    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(2, todos.len());

    // Remove a todo from the list.
    user.todos().remove(&db, &todos[0]).await.unwrap();

    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos_reloaded.len());
    assert_eq!(todos[1].id, todos_reloaded[0].id);

    // Can't find the TODO via a scoped query either
    assert_err!(user.todos().get_by_id(&db, &todos[0].id).await);

    // The todo is not deleted, but user is now None
    let todo = db::Todo::get_by_id(&db, &todos[0].id).await.unwrap();
    assert_none!(todo.user_id);

    // Create a second user w/ a TODO. We will ensure that unlinking *only*
    // unlinks records currently associated with the base model.
    let u2 = db::User::create()
        .todo(db::Todo::create())
        .exec(&db)
        .await
        .unwrap();
    let u2_todos = u2.todos().collect::<Vec<_>>(&db).await.unwrap();

    // Try unlinking u2's todo via user. This should fail
    assert_err!(user.todos().remove(&db, &u2_todos[0]).await);

    // Reload u2's todo
    let u2_todo = db::Todo::get_by_id(&db, &u2_todos[0].id).await.unwrap();
    assert_eq!(*u2_todo.user_id.as_ref().unwrap(), u2.id);

    // Link the TODO back up
    user.todos().insert(&db, &todos[0]).await.unwrap();

    // The TODO is in the association again
    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert!(todos_reloaded
        .iter()
        .find(|t| t.id == todos[0].id)
        .is_some());
    assert_ok!(user.todos().get_by_id(&db, &todos[0].id).await);
}

async fn add_remove_single_relation_required_belongs_to(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with no todos
    let user = db::User::create().exec(&db).await.unwrap();

    // Create some TODOs
    let t1 = user.todos().create().exec(&db).await.unwrap();
    let t2 = user.todos().create().exec(&db).await.unwrap();
    let t3 = user.todos().create().exec(&db).await.unwrap();

    let ids = vec![t1.id.clone(), t2.id.clone(), t3.id.clone()];

    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().find(|t| t.id == id).is_some());
    }

    // Unlinking a todo deletes it
    user.todos().remove(&db, &todos_reloaded[0]).await.unwrap();

    // The TODO no longer exists
    assert_err!(db::Todo::get_by_id(&db, &todos_reloaded[0].id).await);

    // Rest of the todos exist
    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 2);
}

async fn reassign_relation_required_belongs_to(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create users with no todos
    let u1 = db::User::create().exec(&db).await.unwrap();
    let u2 = db::User::create().exec(&db).await.unwrap();

    // Create a TODO associated with user 1
    let t1 = u1.todos().create().exec(&db).await.unwrap();

    // Associate the todo with user 2
    u2.todos().insert(&db, &t1).await.unwrap();

    // The TODO is no longer associated with user 1
    assert!(u1.todos().collect::<Vec<_>>(&db).await.unwrap().is_empty());

    // The TODO is assiated with user 2
    let todos = u2.todos().collect::<Vec<_>>(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(t1.id, todos[0].id);
}

async fn add_remove_multiple_relation_option_belongs_to(s: impl Setup) {
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
            user_id: Option<Id<User>>,

            #[relation(key = user_id, references = id)]
            user: Option<User>,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with no todos
    let user = db::User::create().exec(&db).await.unwrap();

    // Create some TODOs
    let t1 = db::Todo::create().exec(&db).await.unwrap();
    let t2 = db::Todo::create().exec(&db).await.unwrap();
    let t3 = db::Todo::create().exec(&db).await.unwrap();

    let ids = vec![t1.id.clone(), t2.id.clone(), t3.id.clone()];

    // Associate the todos with the user
    user.todos().insert(&db, &[t1, t2, t3]).await.unwrap();

    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().find(|t| t.id == id).is_some());
    }
}

tests!(
    remove_add_single_relation_option_belongs_to,
    add_remove_single_relation_required_belongs_to,
    reassign_relation_required_belongs_to,
    add_remove_multiple_relation_option_belongs_to,
);
