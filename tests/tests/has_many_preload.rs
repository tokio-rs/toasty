use tests::{models, tests, DbTest};
use toasty::stmt::Id;

async fn basic_has_many_and_belongs_to_preload(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        #[has_many]
        todos: toasty::HasMany<Todo>,
    }

    #[derive(Debug, toasty::Model)]
    struct Todo {
        #[key]
        #[auto]
        id: Id<Self>,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Todo)).await;

    // Create a user with a few todos
    let user = User::create()
        .todo(Todo::create())
        .todo(Todo::create())
        .todo(Todo::create())
        .exec(&db)
        .await
        .unwrap();

    // Find the user, include TODOs
    let user = User::filter_by_id(&user.id)
        .include(User::FIELDS.todos())
        .get(&db)
        .await
        .unwrap();

    // This will panic
    assert_eq!(3, user.todos.get().len());

    let id = user.todos.get()[0].id.clone();

    let todo = Todo::filter_by_id(&id)
        .include(Todo::FIELDS.user())
        .get(&db)
        .await
        .unwrap();

    assert_eq!(user.id, todo.user.get().id);
    assert_eq!(user.id, todo.user_id);
}

async fn multiple_includes_same_model(test: &mut DbTest) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[has_many]
        posts: toasty::HasMany<Post>,

        #[has_many]
        comments: toasty::HasMany<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: Id<Self>,

        title: String,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        #[auto]
        id: Id<Self>,

        text: String,

        #[index]
        user_id: Id<User>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let db = test.setup_db(models!(User, Post, Comment)).await;

    // Create a user
    let user = User::create().name("Test User").exec(&db).await.unwrap();

    // Create posts associated with the user
    Post::create()
        .title("Post 1")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Post::create()
        .title("Post 2")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Create comments associated with the user
    Comment::create()
        .text("Comment 1")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Comment::create()
        .text("Comment 2")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    Comment::create()
        .text("Comment 3")
        .user(&user)
        .exec(&db)
        .await
        .unwrap();

    // Test individual includes work (baseline)
    let user_with_posts = User::filter_by_id(&user.id)
        .include(User::FIELDS.posts())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(2, user_with_posts.posts.get().len());

    let user_with_comments = User::filter_by_id(&user.id)
        .include(User::FIELDS.comments())
        .get(&db)
        .await
        .unwrap();
    assert_eq!(3, user_with_comments.comments.get().len());

    println!("---");

    // Test multiple includes in one query
    let loaded_user = User::filter_by_id(&user.id)
        .include(User::FIELDS.posts()) // First include
        .include(User::FIELDS.comments()) // Second include
        .get(&db)
        .await
        .unwrap();

    assert_eq!(2, loaded_user.posts.get().len());
    assert_eq!(3, loaded_user.comments.get().len());
}

tests!(
    basic_has_many_and_belongs_to_preload,
    multiple_includes_same_model
);
