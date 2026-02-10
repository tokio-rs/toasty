//! Test scoped queries on has_many associations

use crate::prelude::*;
use std::collections::HashSet;

#[driver_test(id(ID), matrix(single, composite), requires(or(single, not(id_u64))))]
pub async fn scoped_query_eq(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[driver_test_cfg(composite, key(partition = user_id, local = id))]
    struct Todo {
        #[auto]
        #[driver_test_cfg(single, key)]
        id: ID,

        #[driver_test_cfg(single, index)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,

        order: i64,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create some users
    let u1 = User::create().exec(&db).await.unwrap();
    let u2 = User::create().exec(&db).await.unwrap();

    let mut u1_todo_ids = vec![];

    // Create some TODOs for user 1
    for (order, title) in [
        "write more tests",
        "finish Toasty",
        "remove all todo! items",
        "retire",
    ]
    .into_iter()
    .enumerate()
    {
        let todo = u1
            .todos()
            .create()
            .title(title)
            .order(order as i64)
            .exec(&db)
            .await
            .unwrap();
        u1_todo_ids.push(todo.id);
    }

    // Create a TODO for user 2
    let u2_todo = u2
        .todos()
        .create()
        .title("attend world cup")
        .order(0)
        .exec(&db)
        .await
        .unwrap();

    // Query todos scoped by user 1
    let todos = u1
        .todos()
        .query(Todo::fields().order().eq(0))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();

    assert_eq!(1, todos.len());
    assert_eq!(todos[0].id, u1_todo_ids[0]);
    assert_eq!(todos[0].order, 0);
    assert_eq!(todos[0].title, "write more tests");

    // Querying todos scoped by user 2
    let todos = u2
        .todos()
        .query(Todo::fields().order().eq(0))
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, todos.len());
    assert_eq!(todos[0].id, u2_todo.id);

    // Add a second TODO w/ order 0
    let order_0_todo = u1
        .todos()
        .create()
        .title("another order 0 TODO")
        .order(0)
        .exec(&db)
        .await
        .unwrap();

    let mut actual = HashSet::new();

    // Query for order 0 todos again
    let mut todos = u1
        .todos()
        .query(Todo::fields().order().eq(0))
        .all(&db)
        .await
        .unwrap();

    while let Some(todo) = todos.next().await {
        assert!(actual.insert(todo.unwrap().id));
    }

    let expect: HashSet<_> = [u1_todo_ids[0], order_0_todo.id].into_iter().collect();

    assert_eq!(expect, actual);

    // Query for non-existent TODOs
    let todos = u2
        .todos()
        .query(Todo::fields().order().eq(1))
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(todos.is_empty());
}

#[driver_test(id(ID), matrix(single, composite), requires(or(single, not(id_u64))))]
pub async fn scoped_query_gt(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    #[driver_test_cfg(composite, key(partition = user_id, local = id))]
    struct Todo {
        #[auto]
        #[driver_test_cfg(single, key)]
        id: ID,

        #[driver_test_cfg(single, index)]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,

        title: String,

        order: i64,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    let user = User::create().exec(&db).await.unwrap();

    let todos = Todo::create_many()
        .item(Todo::create().user(&user).title("First").order(0))
        .item(Todo::create().user(&user).title("Second").order(1))
        .item(Todo::create().user(&user).title("Third").order(2))
        .item(Todo::create().user(&user).title("Fourth").order(3))
        .item(Todo::create().user(&user).title("Fifth").order(4))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(5, todos.len());

    // Find all != 2
    let todos: Vec<_> = user
        .todos()
        .query(Todo::fields().order().ne(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["First", "Second", "Fourth", "Fifth"]
    );

    for todo in &todos {
        assert_ne!(todo.order, 2);
    }

    // Find all greater than 2
    let todos: Vec<_> = user
        .todos()
        .query(Todo::fields().order().gt(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["Fourth", "Fifth"]
    );

    // Find all greater than or equal to 2
    let todos: Vec<_> = user
        .todos()
        .query(Todo::fields().order().ge(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["Third", "Fourth", "Fifth"]
    );

    // Find all less than to 2
    let todos: Vec<_> = user
        .todos()
        .query(Todo::fields().order().lt(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["First", "Second"]
    );

    // Find all less than or equal to 2
    let todos: Vec<_> = user
        .todos()
        .query(Todo::fields().order().le(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["First", "Second", "Third"]
    );
}
