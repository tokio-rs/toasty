use crate::prelude::*;

use std::{rc::Rc, sync::Arc};

#[driver_test(requires(sql))]
pub async fn boxed_u64_fk_crud(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,
        #[index]
        user_id: Box<u64>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create a user
    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    // No todos yet
    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert!(todos.is_empty());

    // Create a todo via the has_many association
    let todo = user
        .todos()
        .create()
        .title("Buy groceries")
        .exec(&mut db)
        .await?;

    assert_eq!(todo.title, "Buy groceries");
    assert_eq!(*todo.user_id, user.id);

    // Query back by FK
    let todos: Vec<_> = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "Buy groceries");

    // Create another todo directly
    let todo2 = Todo::create()
        .user(&user)
        .title("Walk the dog")
        .exec(&mut db)
        .await?;

    assert_eq!(*todo2.user_id, user.id);

    // List all user's todos
    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 2);

    // Look up user from todo FK
    let found_user = User::get_by_id(&mut db, &*todo.user_id).await?;
    assert_eq!(found_user.id, user.id);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn boxed_u64_fk_batch_create(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,
        #[index]
        user_id: Box<u64>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    // Create user with todos in one batch
    let user = User::create()
        .name("Bob")
        .todo(Todo::create().title("First task"))
        .todo(Todo::create().title("Second task"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 2);

    for todo in &todos {
        assert_eq!(*todo.user_id, user.id);
    }

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn boxed_u64_fk_delete_and_update(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,
        #[index]
        user_id: Box<u64>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = toasty::create!(User { name: "Carol" })
        .exec(&mut db)
        .await?;

    let mut todo = user
        .todos()
        .create()
        .title("Original")
        .exec(&mut db)
        .await?;

    // Update the todo title
    todo.update().title("Updated").exec(&mut db).await?;

    let reloaded = Todo::get_by_id(&mut db, &todo.id).await?;
    assert_eq!(reloaded.title, "Updated");
    assert_eq!(*reloaded.user_id, user.id);

    // Delete the todo
    reloaded.delete().exec(&mut db).await?;

    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert!(todos.is_empty());

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn arc_u64_fk_crud(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,
        #[index]
        user_id: Arc<u64>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    // Create via association
    let todo = user
        .todos()
        .create()
        .title("Arc task")
        .exec(&mut db)
        .await?;

    assert_eq!(*todo.user_id, user.id);

    // Query back by FK
    let todos: Vec<_> = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "Arc task");

    // Create directly
    let todo2 = Todo::create()
        .user(&user)
        .title("Arc task 2")
        .exec(&mut db)
        .await?;

    assert_eq!(*todo2.user_id, user.id);

    // Batch create
    let user2 = User::create()
        .name("Bob")
        .todo(Todo::create().title("Batch 1"))
        .todo(Todo::create().title("Batch 2"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user2.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 2);

    for todo in &todos {
        assert_eq!(*todo.user_id, user2.id);
    }

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn rc_u64_fk_crud(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: u64,
        name: String,
        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: u64,
        #[index]
        user_id: Rc<u64>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
        title: String,
    }

    let mut db = test.setup_db(models!(User, Todo)).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;

    // Create via association
    let todo = user.todos().create().title("Rc task").exec(&mut db).await?;

    assert_eq!(*todo.user_id, user.id);

    // Query back by FK
    let todos: Vec<_> = Todo::filter_by_user_id(user.id).exec(&mut db).await?;
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "Rc task");

    // Create directly
    let todo2 = Todo::create()
        .user(&user)
        .title("Rc task 2")
        .exec(&mut db)
        .await?;

    assert_eq!(*todo2.user_id, user.id);

    // Batch create
    let user2 = User::create()
        .name("Bob")
        .todo(Todo::create().title("Batch 1"))
        .todo(Todo::create().title("Batch 2"))
        .exec(&mut db)
        .await?;

    let todos: Vec<_> = user2.todos().exec(&mut db).await?;
    assert_eq!(todos.len(), 2);

    for todo in &todos {
        assert_eq!(*todo.user_id, user2.id);
    }

    Ok(())
}
