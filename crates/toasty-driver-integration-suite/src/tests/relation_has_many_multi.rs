//! Test has_many associations with multiple relations to the same model

use crate::prelude::*;
use std::collections::HashMap;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_multi_relation))]
pub async fn crud_user_todos_categories(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Create a user
    let user = User::create().name("Ann Chovey").exec(&mut db).await?;

    // No TODOs
    assert!(user.todos().exec(&mut db).await?.is_empty());

    // Create a category
    let category = Category::create().name("Food").exec(&mut db).await?;

    let mut todos = vec![];

    // Create some TODOs using the different builders
    todos.push(
        user.todos()
            .create()
            .title("one")
            .category(&category)
            .exec(&mut db)
            .await?,
    );

    todos.push(
        Todo::create()
            .title("two")
            .user(&user)
            .category(&category)
            .exec(&mut db)
            .await?,
    );

    todos.push(
        category
            .todos()
            .create()
            .title("three")
            .user(&user)
            .exec(&mut db)
            .await?,
    );

    let expect: HashMap<_, _> = todos.into_iter().map(|todo| (todo.id, todo)).collect();

    let lists = [
        category.todos().exec(&mut db).await?,
        user.todos().exec(&mut db).await?,
        Todo::filter_by_user_id(user.id).exec(&mut db).await?,
    ];

    for list in lists {
        assert_eq!(3, list.len());

        let actual: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();
        assert_eq!(3, actual.len());

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);

            let user = actual.user().exec(&mut db).await?;
            assert_eq!(user.name, "Ann Chovey");
        }
    }

    // Create another user and category
    let user2 = User::create().name("Not ann").exec(&mut db).await?;
    let category2 = Category::create().name("drink").exec(&mut db).await?;

    category
        .todos()
        .create()
        .user(&user2)
        .title("NOPE")
        .exec(&mut db)
        .await?;
    user.todos()
        .create()
        .category(&category2)
        .title("FAIL")
        .exec(&mut db)
        .await?;

    async fn check_todo_list(
        db: &mut toasty::Db,
        expect: &HashMap<ID, Todo>,
        list: Vec<Todo>,
    ) -> Result<()> {
        assert_eq!(3, list.len(), "list={list:#?}");

        let actual: HashMap<_, _> = list.into_iter().map(|todo| (todo.id, todo)).collect();

        assert_eq!(3, actual.len(), "actual={actual:#?}");

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);
            let category = actual.category().exec(db).await?;
            assert_eq!(category.name, "Food");
        }
        Ok(())
    }

    let list = category
        .todos()
        .query(Todo::fields().user().eq(&user))
        .exec(&mut db)
        .await?;
    check_todo_list(&mut db, &expect, list).await?;

    let list = user
        .todos()
        .query(Todo::fields().category().eq(&category))
        .exec(&mut db)
        .await?;
    check_todo_list(&mut db, &expect, list).await?;

    let list = Todo::filter_by_user_id(user.id)
        .filter(Todo::fields().category().eq(&category))
        .exec(&mut db)
        .await?;
    check_todo_list(&mut db, &expect, list).await
}
