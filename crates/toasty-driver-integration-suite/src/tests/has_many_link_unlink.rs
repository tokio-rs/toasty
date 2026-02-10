//! Test linking and unlinking has_many associations

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn remove_add_single_relation_option_belongs_to(test: &mut Test) {
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
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with some todos
    let user = User::create()
        .todo(Todo::create())
        .todo(Todo::create())
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
    let todo = Todo::get_by_id(&db, todos[0].id).await.unwrap();
    assert_none!(todo.user_id);

    // Create a second user w/ a TODO. We will ensure that unlinking *only*
    // unlinks records currently associated with the base model.
    let u2 = User::create().todo(Todo::create()).exec(&db).await.unwrap();
    let u2_todos = u2.todos().collect::<Vec<_>>(&db).await.unwrap();

    // Try unlinking u2's todo via user. This should fail
    assert_err!(user.todos().remove(&db, &u2_todos[0]).await);

    // Reload u2's todo
    let u2_todo = Todo::get_by_id(&db, u2_todos[0].id).await.unwrap();
    assert_eq!(*u2_todo.user_id.as_ref().unwrap(), u2.id);

    // Link the TODO back up
    user.todos().insert(&db, &todos[0]).await.unwrap();

    // The TODO is in the association again
    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert!(todos_reloaded.iter().any(|t| t.id == todos[0].id));
    assert_ok!(user.todos().get_by_id(&db, todos[0].id).await);
}

#[driver_test(id(ID))]
pub async fn add_remove_single_relation_required_belongs_to(test: &mut Test) {
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
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with no todos
    let user = User::create().exec(&db).await.unwrap();

    // Create some TODOs
    let t1 = user.todos().create().exec(&db).await.unwrap();
    let t2 = user.todos().create().exec(&db).await.unwrap();
    let t3 = user.todos().create().exec(&db).await.unwrap();

    let ids = vec![t1.id, t2.id, t3.id];

    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().any(|t| t.id == id));
    }

    // Unlinking a todo deletes it
    user.todos().remove(&db, &todos_reloaded[0]).await.unwrap();

    // The TODO no longer exists
    assert_err!(Todo::get_by_id(&db, todos_reloaded[0].id).await);

    // Rest of the todos exist
    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 2);
}

#[driver_test(id(ID))]
pub async fn reassign_relation_required_belongs_to(test: &mut Test) {
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
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create users with no todos
    let u1 = User::create().exec(&db).await.unwrap();
    let u2 = User::create().exec(&db).await.unwrap();

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

#[driver_test(id(ID))]
pub async fn add_remove_multiple_relation_option_belongs_to(test: &mut Test) {
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
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<Option<User>>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with no todos
    let user = User::create().exec(&db).await.unwrap();

    // Create some TODOs
    let t1 = Todo::create().exec(&db).await.unwrap();
    let t2 = Todo::create().exec(&db).await.unwrap();
    let t3 = Todo::create().exec(&db).await.unwrap();

    let ids = vec![t1.id, t2.id, t3.id];

    // Associate the todos with the user
    user.todos().insert(&db, &[t1, t2, t3]).await.unwrap();

    let todos_reloaded: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(todos_reloaded.len(), 3);

    for id in ids {
        assert!(todos_reloaded.iter().any(|t| t.id == id));
    }
}
