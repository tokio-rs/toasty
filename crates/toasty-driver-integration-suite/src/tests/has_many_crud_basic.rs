//! Test basic has_many associations without any preloading of associations
//! during query time. All associations are accessed via queries on demand.

use crate::prelude::*;
use std::collections::HashMap;

#[driver_test(id(ID))]
pub async fn crud_user_todos(test: &mut Test) {
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&db).await.unwrap();

    // No TODOs
    assert_eq!(
        0,
        user.todos()
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap()
            .len()
    );

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find the todo by ID
    let list = Todo::filter_by_id(todo.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = Todo::filter_by_user_id(user.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the User using the Todo
    let user_reload = User::get_by_id(&db, &todo.user_id).await.unwrap();
    assert_eq!(user.id, user_reload.id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id];
    created.insert(todo.id, todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&db).await.unwrap()
        } else {
            // Create via todo builder
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&db)
                .await
                .unwrap()
        };

        ids.push(todo.id);
        assert_none!(created.insert(todo.id, todo));
    }

    // Load all TODOs
    let list = user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = Todo::filter_by_user_id(user.id)
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = User::create().exec(&db).await.unwrap();

    // No TODOs associated with `user2`
    assert_eq!(
        0,
        user2
            .todos()
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap()
            .len()
    );

    // Create a TODO for user2
    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&db)
        .await
        .unwrap();

    {
        let mut u1_todos = user.todos().all(&db).await.unwrap();

        while let Some(todo) = u1_todos.next().await {
            let todo = todo.unwrap();
            assert_ne!(u2_todo.id, todo.id);
        }
    }

    // Delete a TODO by value
    let todo = Todo::get_by_id(&db, &ids[0]).await.unwrap();
    todo.delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_id(&db, &ids[0]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos().filter_by_id(ids[1]).delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_id(&db, &ids[1]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[1]).await);

    // Successfuly a todo by scope
    user.todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 1")
        .exec(&db)
        .await
        .unwrap();

    let todo = Todo::get_by_id(&db, &ids[2]).await.unwrap();
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 2")
        .exec(&db)
        .await
        .unwrap();

    let todo = Todo::get_by_id(&db, &ids[2]).await.unwrap();
    assert_eq!(todo.title, "batch update 1");

    let id = user.id;

    // Delete the user and associated TODOs are deleted
    user.delete(&db).await.unwrap();
    assert_err!(User::get_by_id(&db, &id).await);
    assert_err!(Todo::get_by_id(&db, &ids[2]).await);
}

#[driver_test(id(ID))]
pub async fn has_many_insert_on_update(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,

        name: String,
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user, no TODOs
    let mut user = User::create().name("Alice").exec(&db).await.unwrap();
    assert!(user
        .todos()
        .collect::<Vec<_>>(&db)
        .await
        .unwrap()
        .is_empty());

    // Update the user and create a todo in a batch
    user.update()
        .name("Bob")
        .todo(Todo::create().title("change name"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!("Bob", user.name);
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todos[0].title, "change name");
}

#[driver_test(id(ID))]
pub async fn scoped_find_by_id(test: &mut Test) {
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a couple of users
    let user1 = User::create().exec(&db).await.unwrap();
    let user2 = User::create().exec(&db).await.unwrap();

    // Create a todo
    let todo = user1
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find it scoped by user1
    let reloaded = user1.todos().get_by_id(&db, &todo.id).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Trying to find the same todo scoped by user2 is missing
    assert_none!(user2
        .todos()
        .filter_by_id(todo.id)
        .first(&db)
        .await
        .unwrap());

    let reloaded = User::filter_by_id(user1.id)
        .todos()
        .get_by_id(&db, &todo.id)
        .await
        .unwrap();

    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Deleting the TODO from the user 2 scope fails
    user2
        .todos()
        .filter_by_id(todo.id)
        .delete(&db)
        .await
        .unwrap();
    let reloaded = user1.todos().get_by_id(&db, &todo.id).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
}

// The has_many association uses the target's primary key as the association's
// foreign key. In this case, the relation's query should not be duplicated.
#[driver_test(id(ID))]
pub async fn has_many_on_target_pk(_test: &mut Test) {}

// The target model has an explicit index on (FK, PK). In this case, the query
// generated by the (FK, PK) pair should not be duplicated by the relation.
#[driver_test(id(ID))]
pub async fn has_many_when_target_indexes_fk_and_pk(_test: &mut Test) {}

// When the FK is composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_fk_is_composite(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = user_id, local = id)]
    struct Todo {
        #[auto]
        id: uuid::Uuid,

        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = User::create().exec(&db).await.unwrap();

    // No TODOs
    assert_eq!(
        0,
        user.todos()
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap()
            .len()
    );

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find the todo by ID
    let list = Todo::filter_by_user_id_and_id(user.id, todo.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = Todo::filter_by_user_id(user.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id];
    created.insert(todo.id, todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&db).await.unwrap()
        } else {
            // Create via todo builder
            Todo::create()
                .user(&user)
                .title(title)
                .exec(&db)
                .await
                .unwrap()
        };

        ids.push(todo.id);
        assert_none!(created.insert(todo.id, todo));
    }

    // Load all TODOs
    let list = user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(6, list.len());

    let loaded: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = Todo::filter_by_user_id(user.id)
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = User::create().exec(&db).await.unwrap();

    // No TODOs associated with `user2`
    assert_eq!(
        0,
        user2
            .todos()
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap()
            .len()
    );

    // Create a TODO for user2
    let u2_todo = user2
        .todos()
        .create()
        .title("user 2 todo")
        .exec(&db)
        .await
        .unwrap();

    let mut u1_todos = user.todos().all(&db).await.unwrap();

    while let Some(todo) = u1_todos.next().await {
        let todo = todo.unwrap();
        assert_ne!(u2_todo.id, todo.id);
    }

    // Delete a TODO by value
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[0])
        .await
        .unwrap();
    todo.delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&db, &user.id, &ids[0]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[0]).await);

    // Delete a TODO by scope
    user.todos().filter_by_id(ids[1]).delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(Todo::get_by_user_id_and_id(&db, &user.id, &ids[1]).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().get_by_id(&db, &ids[1]).await);

    // Successfuly a todo by scope
    user.todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 1")
        .exec(&db)
        .await
        .unwrap();
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[2])
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .filter_by_id(ids[2])
        .update()
        .title("batch update 2")
        .exec(&db)
        .await
        .unwrap();
    let todo = Todo::get_by_user_id_and_id(&db, &user.id, &ids[2])
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");
}

// When the PK is composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_pk_is_composite(_test: &mut Test) {}

// When both the FK and PK are composite, things should still work
#[driver_test(id(ID))]
pub async fn has_many_when_fk_and_pk_are_composite(_test: &mut Test) {}

#[driver_test(id(ID))]
pub async fn belongs_to_required(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        #[allow(dead_code)]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: ID,

        #[index]
        #[allow(dead_code)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        #[allow(dead_code)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    assert_err!(Todo::create().exec(&db).await);
}

#[driver_test(id(ID))]
pub async fn delete_when_belongs_to_optional(test: &mut Test) {
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

    let user = User::create().exec(&db).await.unwrap();
    let mut ids = vec![];

    for _ in 0..3 {
        let todo = user.todos().create().exec(&db).await.unwrap();
        ids.push(todo.id);
    }

    // Delete the user
    user.delete(&db).await.unwrap();

    // All the todos still exist and `user` is set to `None`.
    for id in ids {
        let todo = Todo::get_by_id(&db, id).await.unwrap();
        assert_none!(todo.user_id);
    }

    // Deleting a user leaves hte todo in place.
}

#[driver_test(id(ID))]
pub async fn associate_new_user_with_todo_on_update_via_creation(test: &mut Test) {
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with a todo
    let u1 = User::create()
        .todo(Todo::create().title("hello world"))
        .exec(&db)
        .await
        .unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let mut todo = todos.into_iter().next().unwrap();

    todo.update().user(User::create()).exec(&db).await.unwrap();
}

#[driver_test(id(ID))]
pub async fn associate_new_user_with_todo_on_update_query_via_creation(test: &mut Test) {
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

    // Create a user with a todo
    let u1 = User::create().todo(Todo::create()).exec(&db).await.unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    Todo::filter_by_id(todo.id)
        .update()
        .user(User::create())
        .exec(&db)
        .await
        .unwrap();
}

#[driver_test(id(ID))]
#[should_panic]
pub async fn update_user_with_null_todo_is_err(test: &mut Test) {
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

    use toasty::stmt::{self, IntoExpr};

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with a todo
    let u1 = User::create().todo(Todo::create()).exec(&db).await.unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    // Updating the todo w/ null is an error. Thus requires a bit of a hack to make work
    let mut stmt: stmt::Update<Todo> =
        stmt::Update::new(stmt::Select::from_expr((&todo).into_expr()));
    stmt.set(2, toasty_core::stmt::Value::Null);
    let _ = db.exec(stmt.into()).await.unwrap();

    // User is not deleted
    let u1_reloaded = User::get_by_id(&db, &u1.id).await.unwrap();
    assert_eq!(u1_reloaded.id, u1.id);
}

#[driver_test(id(ID))]
pub async fn assign_todo_that_already_has_user_on_create(test: &mut Test) {
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

    let todo = Todo::create().user(User::create()).exec(&db).await.unwrap();

    let u1 = todo.user().get(&db).await.unwrap();

    let u2 = User::create().todo(&todo).exec(&db).await.unwrap();

    let todo_reload = Todo::get_by_id(&db, &todo.id).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

#[driver_test(id(ID))]
pub async fn assign_todo_that_already_has_user_on_update(test: &mut Test) {
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

    let todo = Todo::create().user(User::create()).exec(&db).await.unwrap();

    let u1 = todo.user().get(&db).await.unwrap();

    let mut u2 = User::create().exec(&db).await.unwrap();

    // Update the user
    u2.update().todo(&todo).exec(&db).await.unwrap();

    let todo_reload = Todo::get_by_id(&db, &todo.id).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

#[driver_test(id(ID))]
pub async fn assign_existing_user_to_todo(test: &mut Test) {
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    let mut todo = Todo::create()
        .title("hello")
        .user(User::create())
        .exec(&db)
        .await
        .unwrap();

    let u1 = todo.user().get(&db).await.unwrap();

    let u2 = User::create().exec(&db).await.unwrap();

    // Update the todo
    todo.update().user(&u2).exec(&db).await.unwrap();

    let todo_reload = Todo::get_by_id(&db, &todo.id).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(0, todos.len());

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

#[driver_test(id(ID))]
pub async fn assign_todo_to_user_on_update_query(test: &mut Test) {
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

        title: String,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&db).await.unwrap();

    User::filter_by_id(user.id)
        .update()
        .todo(Todo::create().title("hello"))
        .exec(&db)
        .await
        .unwrap();

    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("hello", todos[0].title);
}
