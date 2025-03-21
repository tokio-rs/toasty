use tests::*;

use std::collections::HashMap;
use toasty::stmt::Id;

async fn crud_user_todos_categories(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: [Todo],
    }

    #[derive(Debug)]
    #[toasty::model]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: User,

        #[index]
        category_id: Id<Category>,

        #[belongs_to(key = category_id, references = id)]
        category: Category,

        title: String,
    }

    #[derive(Debug)]
    #[toasty::model]
    struct Category {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        todos: [Todo],
    }

    let db = s.setup(models!(User, Todo, Category)).await;

    // Create a user
    let user = User::create().name("Ann Chovey").exec(&db).await.unwrap();

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

    let expect: HashMap<_, _> = todos
        .into_iter()
        .map(|todo| (todo.id.clone(), todo))
        .collect();

    let lists = [
        category.todos().collect::<Vec<_>>(&db).await.unwrap(),
        user.todos().collect::<Vec<_>>(&db).await.unwrap(),
        Todo::filter_by_user_id(&user.id)
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

    fn check_todo_list(expect: &HashMap<Id<Todo>, Todo>, list: Vec<Todo>) {
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
            .query(Todo::FIELDS.user.eq(&user))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );

    check_todo_list(
        &expect,
        user.todos()
            .query(Todo::FIELDS.category.eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );

    check_todo_list(
        &expect,
        Todo::filter_by_user_id(&user.id)
            .filter(Todo::FIELDS.category.eq(&category))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap(),
    );
}

tests!(crud_user_todos_categories,);
