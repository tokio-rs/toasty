//! Test basic has_many associations without any preloading of associations
//! during query time. All associations are accessed via queries on demand.

use tests_client::*;

use std::collections::HashMap;
use std_util::prelude::*;

async fn crud_user_todos(s: impl Setup) {
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

            #[relation(key = user_id, references = id)]
            user: User,

            #[index]
            user_id: Id<User>,

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user
    let user = db::User::create().exec(&db).await.unwrap();

    // No TODOs
    assert_empty!(user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find the todo by ID
    let list = db::Todo::find_by_id(&todo.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = db::Todo::find_by_user_id(&user.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the User using the Todo
    let user_reload = db::User::find_by_id(&todo.user_id).get(&db).await.unwrap();
    assert_eq!(user.id, user_reload.id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id.clone()];
    created.insert(todo.id.clone(), todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&db).await.unwrap()
        } else {
            // Create via todo builder
            db::Todo::create()
                .user(&user)
                .title(title)
                .exec(&db)
                .await
                .unwrap()
        };

        ids.push(todo.id.clone());
        assert_none!(created.insert(todo.id.clone(), todo));
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

    let loaded: HashMap<_, _> = list
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = db::Todo::find_by_user_id(&user.id)
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = db::User::create().exec(&db).await.unwrap();

    // No TODOs associated with `user2`
    assert_empty!(user2
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

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
    let todo = db::Todo::find_by_id(&ids[0]).get(&db).await.unwrap();
    todo.delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(db::Todo::find_by_id(&ids[0]).get(&db).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().find_by_id(&ids[0]).get(&db).await);

    // Delete a TODO by scope
    user.todos().find_by_id(&ids[1]).delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(db::Todo::find_by_id(&ids[1]).get(&db).await);

    // Can no longer get the todo scoped
    assert_err!(user.todos().find_by_id(&ids[1]).get(&db).await);

    // Successfuly a todo by scope
    user.todos()
        .find_by_id(&ids[2])
        .update()
        .title("batch update 1")
        .exec(&db)
        .await
        .unwrap();
    let todo = db::Todo::find_by_id(&ids[2]).get(&db).await.unwrap();
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .find_by_id(&ids[2])
        .update()
        .title("batch update 2")
        .exec(&db)
        .await
        .unwrap();
    let todo = db::Todo::find_by_id(&ids[2]).get(&db).await.unwrap();
    assert_eq!(todo.title, "batch update 1");

    let id = user.id.clone();

    // Delete the user and associated TODOs are deleted
    user.delete(&db).await.unwrap();
    assert_err!(db::User::find_by_id(&id).get(&db).await);
    assert_err!(db::Todo::find_by_id(&ids[2]).get(&db).await);
}

async fn has_many_insert_on_update(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            todos: [Todo],

            name: String,
        }

        model Todo {
            #[key]
            #[auto]
            id: Id,

            #[relation(key = user_id, references = id)]
            user: User,

            #[index]
            user_id: Id<User>,

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user, no TODOs
    let mut user = db::User::create().name("Alice").exec(&db).await.unwrap();
    assert!(user
        .todos()
        .collect::<Vec<_>>(&db)
        .await
        .unwrap()
        .is_empty());

    // Update the user and create a todo in a batch
    user.update()
        .name("Bob")
        .todo(db::Todo::create().title("change name"))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!("Bob", user.name);
    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todos[0].title, "change name");
}

async fn scoped_find_by_id(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a couple of users
    let user1 = db::User::create().exec(&db).await.unwrap();
    let user2 = db::User::create().exec(&db).await.unwrap();

    // Create a todo
    let todo = user1
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find it scoped by user1
    let reloaded = user1.todos().find_by_id(&todo.id).get(&db).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Trying to find the same todo scoped by user2 is missing
    assert_none!(user2.todos().find_by_id(&todo.id).first(&db).await.unwrap());

    let reloaded = db::User::find_by_id(&user1.id)
        .todos()
        .find_by_id(&todo.id)
        .get(&db)
        .await
        .unwrap();

    assert_eq!(reloaded.id, todo.id);
    assert_eq!(reloaded.title, todo.title);

    // Deleting the TODO from the user 2 scope fails
    user2
        .todos()
        .find_by_id(&todo.id)
        .delete(&db)
        .await
        .unwrap();
    let reloaded = user1.todos().find_by_id(&todo.id).get(&db).await.unwrap();
    assert_eq!(reloaded.id, todo.id);
}

// The has_many association uses the target's primary key as the association's
// foreign key. In this case, the relation's query should not be duplicated.
async fn has_many_on_target_pk(_s: impl Setup) {}

// The target model has an explicit index on (FK, PK). In this case, the query
// generated by the (FK, PK) pair should not be duplicated by the relation.
async fn has_many_when_target_indexes_fk_and_pk(_s: impl Setup) {}

// When the FK is composite, things should still work
async fn has_many_when_fk_is_composite(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            todos: [Todo],
        }

        #[key(partition = user_id, local = id)]
        model Todo {
            #[auto]
            id: Id,

            #[relation(key = user_id, references = id)]
            user: User,

            user_id: Id<User>,

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user
    let user = db::User::create().exec(&db).await.unwrap();

    // No TODOs
    assert_empty!(user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

    // Create a Todo associated with the user
    let todo = user
        .todos()
        .create()
        .title("hello world")
        .exec(&db)
        .await
        .unwrap();

    // Find the todo by ID
    let list = db::Todo::find_by_user_id_and_id(&user.id, &todo.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    // Find the TODO by user ID
    let list = db::Todo::find_by_user_id(&user.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, list.len());
    assert_eq!(todo.id, list[0].id);

    let mut created = HashMap::new();
    let mut ids = vec![todo.id.clone()];
    created.insert(todo.id.clone(), todo);

    // Create a few more TODOs
    for i in 0..5 {
        let title = format!("hello world {i}");

        let todo = if i.is_even() {
            // Create via user
            user.todos().create().title(title).exec(&db).await.unwrap()
        } else {
            // Create via todo builder
            db::Todo::create()
                .user(&user)
                .title(title)
                .exec(&db)
                .await
                .unwrap()
        };

        ids.push(todo.id.clone());
        assert_none!(created.insert(todo.id.clone(), todo));
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

    let loaded: HashMap<_, _> = list
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();
    assert_eq!(6, loaded.len());

    for (id, expect) in &created {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Find all TODOs by user (using the belongs_to queries)
    let list = db::Todo::find_by_user_id(&user.id)
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(6, list.len());

    let by_id: HashMap<_, _> = list
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();

    assert_eq!(6, by_id.len());

    for (id, expect) in by_id {
        assert_eq!(expect.title, loaded[&id].title);
    }

    // Create a second user
    let user2 = db::User::create().exec(&db).await.unwrap();

    // No TODOs associated with `user2`
    assert_empty!(user2
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

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
    let todo = db::Todo::find_by_user_id_and_id(&user.id, &ids[0])
        .get(&db)
        .await
        .unwrap();
    todo.delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(
        db::Todo::find_by_user_id_and_id(&user.id, &ids[0])
            .get(&db)
            .await
    );

    // Can no longer get the todo scoped
    assert_err!(user.todos().find_by_id(&ids[0]).get(&db).await);

    // Delete a TODO by scope
    user.todos().find_by_id(&ids[1]).delete(&db).await.unwrap();

    // Can no longer get the todo via id
    assert_err!(
        db::Todo::find_by_user_id_and_id(&user.id, &ids[1])
            .get(&db)
            .await
    );

    // Can no longer get the todo scoped
    assert_err!(user.todos().find_by_id(&ids[1]).get(&db).await);

    // Successfuly a todo by scope
    user.todos()
        .find_by_id(&ids[2])
        .update()
        .title("batch update 1")
        .exec(&db)
        .await
        .unwrap();
    let todo = db::Todo::find_by_user_id_and_id(&user.id, &ids[2])
        .get(&db)
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");

    // Now fail to update it by scoping by other user
    user2
        .todos()
        .find_by_id(&ids[2])
        .update()
        .title("batch update 2")
        .exec(&db)
        .await
        .unwrap();
    let todo = db::Todo::find_by_user_id_and_id(&user.id, &ids[2])
        .get(&db)
        .await
        .unwrap();
    assert_eq!(todo.title, "batch update 1");
}

// When the PK is composite, things should still work
async fn has_many_when_pk_is_composite(_s: impl Setup) {}

// When both the FK and PK are composite, things should still work
async fn has_many_when_fk_and_pk_are_composite(_s: impl Setup) {}

async fn belongs_to_required(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    db::Todo::create()
        .title("missing user")
        .exec(&db)
        .await
        .unwrap();
}

async fn delete_when_belongs_to_optional(s: impl Setup) {
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

            title: String,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create().exec(&db).await.unwrap();
    let mut ids = vec![];

    for i in 0..3 {
        let todo = user
            .todos()
            .create()
            .title(format!("todo {i}"))
            .exec(&db)
            .await
            .unwrap();
        ids.push(todo.id);
    }

    // Delete the user
    user.delete(&db).await.unwrap();

    // All the todos still exist and `user` is set to `None`.
    for id in ids {
        let todo = db::Todo::find_by_id(id).get(&db).await.unwrap();
        assert_none!(todo.user_id);
    }

    // Deleting a user leaves hte todo in place.
}

async fn associate_new_user_with_todo_on_update_via_creation(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with a todo
    let u1 = db::User::create()
        .todo(db::Todo::create().title("hello world"))
        .exec(&db)
        .await
        .unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let mut todo = todos.into_iter().next().unwrap();

    todo.update()
        .user(db::User::create())
        .exec(&db)
        .await
        .unwrap();
}

async fn associate_new_user_with_todo_on_update_query_via_creation(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with a todo
    let u1 = db::User::create()
        .todo(db::Todo::create().title("hello world"))
        .exec(&db)
        .await
        .unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    db::Todo::find_by_id(&todo.id)
        .update()
        .user(db::User::create())
        .exec(&db)
        .await
        .unwrap();
}

async fn update_user_with_null_todo_is_err(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user with a todo
    let u1 = db::User::create()
        .todo(db::Todo::create().title("hello world"))
        .exec(&db)
        .await
        .unwrap();

    // Get the todo
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    let todo = todos.into_iter().next().unwrap();

    // Updating the todo w/ null is an error. Thus requires a bit of a hack to make work
    let mut stmt = toasty::stmt::Update::new(&todo);
    stmt.set(2, toasty_core::stmt::Value::Null);
    let _ = db.exec(stmt.into()).await.unwrap();

    // User is not deleted
    let u1_reloaded = db::User::find_by_id(&u1.id).get(&db).await.unwrap();
    assert_eq!(u1_reloaded.id, u1.id);
}

async fn assign_todo_that_already_has_user_on_create(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let todo = db::Todo::create()
        .title("hello")
        .user(db::User::create())
        .exec(&db)
        .await
        .unwrap();

    let u1 = todo.user().find(&db).await.unwrap();

    let u2 = db::User::create().todo(&todo).exec(&db).await.unwrap();

    let todo_reload = db::Todo::find_by_id(&todo.id).get(&db).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_empty!(todos);

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

async fn assign_todo_that_already_has_user_on_update(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let todo = db::Todo::create()
        .title("hello")
        .user(db::User::create())
        .exec(&db)
        .await
        .unwrap();

    let u1 = todo.user().find(&db).await.unwrap();

    let mut u2 = db::User::create().exec(&db).await.unwrap();

    // Update the user
    u2.update().todo(&todo).exec(&db).await.unwrap();

    let todo_reload = db::Todo::find_by_id(&todo.id).get(&db).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_empty!(todos);

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

async fn assign_existing_user_to_todo(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let mut todo = db::Todo::create()
        .title("hello")
        .user(db::User::create())
        .exec(&db)
        .await
        .unwrap();

    let u1 = todo.user().find(&db).await.unwrap();

    let u2 = db::User::create().exec(&db).await.unwrap();

    // Update the todo
    todo.update().user(&u2).exec(&db).await.unwrap();

    let todo_reload = db::Todo::find_by_id(&todo.id).get(&db).await.unwrap();

    assert_eq!(u2.id, todo_reload.user_id);

    // First user has no todos
    let todos: Vec<_> = u1.todos().collect(&db).await.unwrap();
    assert_empty!(todos);

    // Second user has the todo
    let todos: Vec<_> = u2.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!(todo.id, todos[0].id);
}

async fn assign_todo_to_user_on_update_query(s: impl Setup) {
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
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create().exec(&db).await.unwrap();

    db::User::find_by_id(&user.id)
        .update()
        .todo(db::Todo::create().title("hello"))
        .exec(&db)
        .await
        .unwrap();

    let todos: Vec<_> = user.todos().collect(&db).await.unwrap();
    assert_eq!(1, todos.len());
    assert_eq!("hello", todos[0].title);
}

tests!(
    crud_user_todos,
    has_many_insert_on_update,
    scoped_find_by_id,
    has_many_on_target_pk,
    has_many_when_target_indexes_fk_and_pk,
    has_many_when_fk_is_composite,
    has_many_when_pk_is_composite,
    has_many_when_fk_and_pk_are_composite,
    #[should_panic] // TODO: Err?
    belongs_to_required,
    delete_when_belongs_to_optional,
    associate_new_user_with_todo_on_update_via_creation,
    associate_new_user_with_todo_on_update_query_via_creation,
    #[should_panic(expected = "invalid statement. handle this case")]
    update_user_with_null_todo_is_err,
    assign_todo_that_already_has_user_on_create,
    assign_todo_that_already_has_user_on_update,
    assign_existing_user_to_todo,
    assign_todo_to_user_on_update_query,
);
