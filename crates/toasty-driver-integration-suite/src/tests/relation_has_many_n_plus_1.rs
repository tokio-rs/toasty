//! Test N+1 query behavior with has_many associations

use crate::prelude::*;
use hashbrown::HashSet;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn hello_world(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let cat1 = Category::create().name("a").exec(&mut db).await?;
    let cat2 = Category::create().name("b").exec(&mut db).await?;

    // Create a user with a few todos
    let user = User::create()
        .name("x")
        .todos([Todo::create().category(&cat1).title("one")])
        .todos([Todo::create().category(&cat2).title("two")])
        .todos([Todo::create().category(&cat2).title("three")])
        .exec(&mut db)
        .await?;

    let todos = user.todos().exec(&mut db).await?;

    assert_eq!(3, todos.len());
    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn query_by_index_optimization(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    if db.capability().sql {
        // Statement count is correct for DDB, but not MySQL
        return Ok(());
    }
    // Create a user with 5 todos
    let user = User::create()
        .name("Test Board")
        .todos([Todo::create().title("User 1")])
        .todos([Todo::create().title("User 2")])
        .todos([Todo::create().title("User 3")])
        .todos([Todo::create().title("User 4")])
        .todos([Todo::create().title("User 5")])
        .exec(&mut db)
        .await?;

    User::create()
        .name("Test Board2")
        .todos([Todo::create().title("User 6")])
        .exec(&mut db)
        .await?;

    // Clear operation log before the query we want to test
    test.log().clear();

    // Query user with .include(todos())
    let user_from_db = User::filter_by_id(user.id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    // Check the logged operations
    let log = test.log();
    let operation_count = log.len();

    assert_eq!(
        operation_count, 2,
        "Expected 2 DynamoDB operations (GetByKey + QueryPk with index), got {:?}",
        log
    );

    // Check todo ids
    let from_db = user_from_db
        .todos
        .get()
        .iter()
        .map(|u| u.id)
        .collect::<HashSet<_>>();
    let expected = user
        .todos
        .get()
        .iter()
        .map(|u| u.id)
        .collect::<HashSet<_>>();
    assert_eq!(from_db, expected);

    Ok(())
}
