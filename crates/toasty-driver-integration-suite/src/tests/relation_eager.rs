use crate::prelude::*;

use toasty::schema::Model;
use toasty_core::stmt;

#[driver_test]
pub async fn eager_has_many_and_has_one_load_without_include(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,
        name: String,

        #[has_many]
        posts: Vec<Post>,

        #[has_one]
        profile: Option<Profile>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    #[derive(Debug, toasty::Model)]
    struct Profile {
        #[key]
        id: uuid::Uuid,
        bio: String,

        #[unique]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = t.setup_db(models!(User, Post, Profile)).await;
    let user_id = uuid::Uuid::from_u128(1);

    insert_row::<User>(
        &mut db,
        vec![
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::from("Alice").into(),
            stmt::Value::Null.into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;
    insert_row::<Post>(
        &mut db,
        vec![
            stmt::Value::Uuid(uuid::Uuid::from_u128(2)).into(),
            stmt::Value::from("hello").into(),
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;
    insert_row::<Profile>(
        &mut db,
        vec![
            stmt::Value::Uuid(uuid::Uuid::from_u128(3)).into(),
            stmt::Value::from("writer").into(),
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;

    let user = User::filter_by_id(user_id).get(&mut db).await?;

    assert_eq!(user.posts.len(), 1);
    assert_eq!(user.posts[0].title, "hello");
    assert_eq!(user.profile.as_ref().unwrap().bio, "writer");

    Ok(())
}

#[driver_test]
pub async fn eager_belongs_to_loads_without_include(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: User,
    }

    let mut db = t.setup_db(models!(User, Post)).await;
    let user_id = uuid::Uuid::from_u128(4);
    let post_id = uuid::Uuid::from_u128(5);

    insert_row::<User>(
        &mut db,
        vec![
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::from("Alice").into(),
        ],
    )
    .await?;
    insert_row::<Post>(
        &mut db,
        vec![
            stmt::Value::Uuid(post_id).into(),
            stmt::Value::from("hello").into(),
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;

    let post = Post::filter_by_id(post_id).get(&mut db).await?;

    assert_eq!(post.title, "hello");
    assert_eq!(post.user.name, "Alice");

    Ok(())
}

#[driver_test(id(ID))]
pub async fn eager_has_many_create_returning_loads_relations(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,

        #[has_many]
        posts: Vec<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,
        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;

    let user = User::create()
        .name("Alice")
        .post(Post::create().title("hello"))
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Alice");
    assert_eq!(user.posts.len(), 1);
    assert_eq!(user.posts[0].title, "hello");

    Ok(())
}

#[driver_test]
pub async fn eager_nested_relations_load_without_include(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,
        name: String,

        #[has_many]
        posts: Vec<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        id: uuid::Uuid,
        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[has_many]
        comments: Vec<Comment>,
    }

    #[derive(Debug, toasty::Model)]
    struct Comment {
        #[key]
        id: uuid::Uuid,
        body: String,

        #[index]
        post_id: uuid::Uuid,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::Deferred<Post>,
    }

    let mut db = t.setup_db(models!(User, Post, Comment)).await;
    let user_id = uuid::Uuid::from_u128(10);
    let post_id = uuid::Uuid::from_u128(11);

    insert_row::<User>(
        &mut db,
        vec![
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::from("Alice").into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;
    insert_row::<Post>(
        &mut db,
        vec![
            stmt::Value::Uuid(post_id).into(),
            stmt::Value::from("hello").into(),
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::Null.into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;
    insert_row::<Comment>(
        &mut db,
        vec![
            stmt::Value::Uuid(uuid::Uuid::from_u128(12)).into(),
            stmt::Value::from("first").into(),
            stmt::Value::Uuid(post_id).into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;

    let user = User::filter_by_id(user_id).get(&mut db).await?;

    assert_eq!(user.posts.len(), 1);
    assert_eq!(user.posts[0].title, "hello");
    assert_eq!(user.posts[0].comments.len(), 1);
    assert_eq!(user.posts[0].comments[0].body, "first");

    Ok(())
}

#[driver_test]
pub async fn eager_relations_reload_after_update(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: uuid::Uuid,
        name: String,

        #[has_many]
        posts: Vec<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,

        #[index]
        user_id: uuid::Uuid,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,
    }

    let mut db = t.setup_db(models!(User, Post)).await;
    let user_id = uuid::Uuid::from_u128(20);

    insert_row::<User>(
        &mut db,
        vec![
            stmt::Value::Uuid(user_id).into(),
            stmt::Value::from("Alice").into(),
            stmt::Value::Null.into(),
        ],
    )
    .await?;

    let mut user = User::filter_by_id(user_id).get(&mut db).await?;
    assert!(user.posts.is_empty());

    user.update()
        .name("Alice Updated")
        .posts(toasty::stmt::insert(Post::create().title("first")))
        .exec(&mut db)
        .await?;

    let mut titles = user
        .posts
        .iter()
        .map(|post| post.title.as_str())
        .collect::<Vec<_>>();
    titles.sort_unstable();

    assert_eq!(user.name, "Alice Updated");
    assert_eq!(titles, vec!["first"]);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn eager_relation_cycle_is_rejected(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        posts: Vec<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: User,
    }

    let err = t.try_setup_db(models!(User, Post)).await.unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("eager relation cycle"),
        "expected eager relation cycle error, got: {msg}"
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn eager_relation_self_cycle_is_rejected(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Node {
        #[key]
        #[auto]
        id: ID,

        #[index]
        parent_id: Option<ID>,

        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::Deferred<Option<Node>>,

        #[has_many(pair = parent)]
        children: Vec<Node>,
    }

    let err = t.try_setup_db(models!(Node)).await.unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("eager relation cycle"),
        "expected eager relation cycle error, got: {msg}"
    );

    Ok(())
}

#[driver_test(id(ID))]
pub async fn eager_relation_long_cycle_is_rejected(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        posts: Vec<Post>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[has_one]
        detail: Option<Detail>,
    }

    #[derive(Debug, toasty::Model)]
    struct Detail {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        post_id: ID,

        #[belongs_to(key = post_id, references = id)]
        post: toasty::Deferred<Post>,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: User,
    }

    let err = t
        .try_setup_db(models!(User, Post, Detail))
        .await
        .unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("eager relation cycle"),
        "expected eager relation cycle error, got: {msg}"
    );

    Ok(())
}

async fn insert_row<M: Model>(db: &mut toasty::Db, fields: Vec<stmt::Expr>) -> Result<()> {
    let insert = stmt::Insert {
        target: stmt::InsertTarget::Model(<M as toasty::schema::Register>::id()),
        source: stmt::Query::new_single(vec![stmt::Expr::record(fields)]),
        returning: None,
    };

    toasty::Statement::<()>::from_untyped_stmt(insert.into())
        .exec(db)
        .await
}
