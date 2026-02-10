//! Test has_many associations with multiple relations to the same model

use crate::prelude::*;
use std::collections::HashMap;

#[driver_test(id(ID))]
pub async fn crud_user_todos_categories(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

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

        name: String,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    let db = test.setup_db(models!(User, Todo, Category)).await;

    // Create a user
    let user = User::create().name("Ann Chovey").exec(&db).await.unwrap();

    // No TODOs
    assert!(user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap()
        .is_empty());

    // Create a category
    let category = Category::create().name("Food").exec(&db).await.unwrap();

    let mut todos = vec![];

    // Create some TODOs using the different builders
    todos.push(
        user.todos()
            .create()
            .title("one")
            .category(&category)
            .exec(&db)
            .await
            .unwrap(),
    );

    todos.push(
        Todo::create()
            .title("two")
            .user(&user)
            .category(&category)
            .exec(&db)
            .await
            .unwrap(),
    );

    todos.push(
        category
            .todos()
            .create()
            .title("three")
            .user(&user)
            .exec(&db)
            .await
            .unwrap(),
    );

    let expect: HashMap<_, _> = todos.into_iter().map(|todo| (todo.id, todo)).collect();

    let lists = [
        category.todos().collect::<Vec<_>>(&db).await.unwrap(),
        user.todos().collect::<Vec<_>>(&db).await.unwrap(),
        Todo::filter_by_user_id(user.id)
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    ];

    for list in lists {
        assert_eq!(3, list.len());

        let actual: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
        assert_eq!(3, actual.len());

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);

            let user = actual.user().get(&db).await.unwrap();
            assert_eq!(user.name, "Ann Chovey");
        }
    }

    // Create another user and category
    let user2 = User::create().name("Not ann").exec(&db).await.unwrap();
    let category2 = Category::create().name("drink").exec(&db).await.unwrap();

    category
        .todos()
        .create()
        .user(&user2)
        .title("NOPE")
        .exec(&db)
        .await
        .unwrap();
    user.todos()
        .create()
        .category(&category2)
        .title("FAIL")
        .exec(&db)
        .await
        .unwrap();

    async fn check_todo_list(db: &toasty::Db, expect: &HashMap<ID, Todo>, list: Vec<Todo>) {
        assert_eq!(3, list.len(), "list={list:#?}");

        let actual: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

        assert_eq!(3, actual.len(), "actual={actual:#?}");

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);
            let category = actual.category().get(db).await.unwrap();
            assert_eq!(category.name, "Food");
        }
    }

    check_todo_list(
        &db,
        &expect,
        category
            .todos()
            .query(Todo::fields().user().eq(&user))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    )
    .await;

    check_todo_list(
        &db,
        &expect,
        user.todos()
            .query(Todo::fields().category().eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    )
    .await;

    check_todo_list(
        &db,
        &expect,
        Todo::filter_by_user_id(user.id)
            .filter(Todo::fields().category().eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    )
    .await;
}
