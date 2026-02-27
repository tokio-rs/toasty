//! Test N+1 query behavior with has_many associations

use crate::prelude::*;
use std::collections::HashSet;

#[driver_test(id(ID))]
pub async fn hello_world(test: &mut Test) -> Result<()> {
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

        #[index]
        category_id: ID,

        #[belongs_to(key = category_id, references = id)]
        category: toasty::BelongsTo<Category>,

        title: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Category {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        #[allow(dead_code)]
        todos: toasty::HasMany<Todo>,

        #[allow(dead_code)]
        name: String,
    }

    let db = test.setup_db(models!(User, Todo, Category)).await;

    let cat1 = Category::create().name("a").exec(&db).await?;
    let cat2 = Category::create().name("b").exec(&db).await?;

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create().category(&cat1).title("one"))
        .todo(Todo::create().category(&cat2).title("two"))
        .todo(Todo::create().category(&cat2).title("three"))
        .exec(&db)
        .await?;

    let todos = user.todos().all(&db).await?;

    let todos: Vec<_> = todos.collect().await?;
    assert_eq!(3, todos.len());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_by_index_optimization(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Board {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        users: toasty::HasMany<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        board_id: ID,

        #[belongs_to(key = board_id, references = id)]
        board: toasty::BelongsTo<Board>,
    }

    let db = test.setup_db(models!(Board, User)).await;
    if db.capability().sql {
        // Statement count is correct for DDB, but not MySQL
        return Ok(());
    }
    // Create a board with 5 users
    let board = Board::create()
        .name("Test Board")
        .user(User::create().name("User 1"))
        .user(User::create().name("User 2"))
        .user(User::create().name("User 3"))
        .user(User::create().name("User 4"))
        .user(User::create().name("User 5"))
        .exec(&db)
        .await?;

    Board::create()
        .name("Test Board2")
        .user(User::create().name("User 6"))
        .exec(&db)
        .await?;

    // Clear operation log before the query we want to test
    test.log().clear();

    // Query board with .include(users())
    let board_from_db = Board::filter_by_id(board.id)
        .include(Board::fields().users())
        .get(&db)
        .await?;

    // Check the logged operations
    let log = test.log();
    let operation_count = log.len();

    assert_eq!(
        operation_count, 2,
        "Expected 2 DynamoDB operations (GetByKey + QueryPk with index), got {:?}",
        log
    );

    // Check board ids
    let from_db = board_from_db
        .users
        .get()
        .iter()
        .map(|u| u.id)
        .collect::<HashSet<_>>();
    let expected = board
        .users
        .get()
        .iter()
        .map(|u| u.id)
        .collect::<HashSet<_>>();
    assert_eq!(from_db, expected);

    Ok(())
}
