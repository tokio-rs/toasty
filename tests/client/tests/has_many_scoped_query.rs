use tests_client::*;

use std::collections::HashSet;

async fn scoped_query_eq(s: impl Setup) {
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

            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            title: String,

            order: i64,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create some users
    let u1 = db::User::create().exec(&db).await.unwrap();
    let u2 = db::User::create().exec(&db).await.unwrap();

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
        u1_todo_ids.push(todo.id.clone());
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
        .query(db::Todo::ORDER.eq(0))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();

    assert_eq!(1, todos.len());
    assert_eq!(todos[0].id, u1_todo_ids[0]);

    // Querying todos scoped by user 2
    let todos = u2
        .todos()
        .query(db::Todo::ORDER.eq(0))
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
        .query(db::Todo::ORDER.eq(0))
        .all(&db)
        .await
        .unwrap();

    while let Some(todo) = todos.next().await {
        assert!(actual.insert(todo.unwrap().id.clone()));
    }

    let expect: HashSet<_> = [u1_todo_ids[0].clone(), order_0_todo.id.clone()]
        .into_iter()
        .collect();

    assert_eq!(expect, actual);

    // Query for non-existent TODOs
    let todos = u2
        .todos()
        .query(db::Todo::ORDER.eq(1))
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert!(todos.is_empty());
}

async fn scoped_query_gt(s: impl Setup) {
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

            user_id: Id<User>,

            #[relation(key = user_id, references = id)]
            user: User,

            title: String,

            order: i64,
        }"
    );

    let db = s.setup(db::load_schema()).await;

    let user = db::User::create().exec(&db).await.unwrap();

    let todos = db::Todo::create_many()
        .item(db::Todo::create().user(&user).title("First").order(0))
        .item(db::Todo::create().user(&user).title("Second").order(1))
        .item(db::Todo::create().user(&user).title("Third").order(2))
        .item(db::Todo::create().user(&user).title("Fourth").order(3))
        .item(db::Todo::create().user(&user).title("Fifth").order(4))
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(5, todos.len());

    // Find all != 2
    let todos: Vec<_> = user
        .todos()
        .query(db::Todo::ORDER.ne(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["First", "Second", "Fourth", "Fifth"]
    );

    // Find all greater than 2
    let todos: Vec<_> = user
        .todos()
        .query(db::Todo::ORDER.gt(2))
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
        .query(db::Todo::ORDER.ge(2))
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
        .query(db::Todo::ORDER.lt(2))
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
        .query(db::Todo::ORDER.le(2))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        todos.iter().map(|todo| &todo.title[..]),
        ["First", "Second", "Third"]
    );
}

tests!(scoped_query_eq, scoped_query_gt,);
