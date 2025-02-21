use tests_client::*;

use std::collections::HashMap;
use toasty::stmt::Id;

async fn crud_user_todos_categories(s: impl Setup) {
    schema!(
        "
        model User {
            #[key]
            #[auto]
            id: Id,

            name: String,

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

            #[index]
            category_id: Id<Category>,

            #[relation(key = category_id, references = id)]
            category: Category,

            title: String,
        }

        model Category {
            #[key]
            #[auto]
            id: Id,

            name: String,

            todos: [Todo],
        }"
    );

    let db = s.setup(db::load_schema()).await;

    // Create a user
    let user = db::User::create()
        .name("Ann Chovey")
        .exec(&db)
        .await
        .unwrap();

    // No TODOs
    assert_empty!(user
        .todos()
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap());

    // Create a category
    let category = db::Category::create().name("Food").exec(&db).await.unwrap();

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
        db::Todo::create()
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

    let expect: HashMap<_, _> = todos
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();

    let lists = [
        category.todos().collect::<Vec<_>>(&db).await.unwrap(),
        user.todos().collect::<Vec<_>>(&db).await.unwrap(),
        db::Todo::filter_by_user_id(&user.id)
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    ];

    for list in lists {
        assert_eq!(3, list.len());

        let actual: HashMap<_, _> = list
            .into_iter()
            .map(|todo| (todo.id.clone(), todo))
            .collect();
        assert_eq!(3, actual.len());

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);
        }
    }

    // Create another user and category
    let user2 = db::User::create().name("Not ann").exec(&db).await.unwrap();
    let category2 = db::Category::create()
        .name("drink")
        .exec(&db)
        .await
        .unwrap();

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

    fn check_todo_list(expect: &HashMap<Id<db::Todo>, db::Todo>, list: Vec<db::Todo>) {
        assert_eq!(3, list.len(), "list={list:#?}");

        let actual: HashMap<_, _> = list
            .into_iter()
            .map(|todo| (todo.id.clone(), todo))
            .collect();

        assert_eq!(3, actual.len(), "actual={actual:#?}");

        for (id, actual) in actual {
            assert_eq!(expect[&id].title, actual.title);
        }
    }

    check_todo_list(
        &expect,
        category
            .todos()
            .query(db::Todo::USER.eq(&user))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );

    check_todo_list(
        &expect,
        user.todos()
            .query(db::Todo::CATEGORY.eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );

    check_todo_list(
        &expect,
        db::Todo::filter_by_user_id(&user.id)
            .filter(db::Todo::CATEGORY.eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );
}

tests!(crud_user_todos_categories,);
