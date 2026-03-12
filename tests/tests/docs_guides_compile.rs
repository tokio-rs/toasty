//! Compile-smoke coverage for additional guide snippets.
//!
//! These tests focus on documented API shapes from guide files that are not
//! fully covered by README-only compile checks.

#![allow(dead_code)]

use toasty::Executor;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    name: String,

    nickname: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    user_id: uuid::Uuid,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    #[index]
    title: String,
}

#[derive(Debug, toasty::Model)]
struct Foo {
    #[key]
    one: String,

    #[key]
    two: String,
}

#[test]
fn relationship_and_batch_builder_surfaces_compile() {
    let _with_todos = User::all().include(User::fields().todos());

    let _with_multiple = Todo::all()
        .include(Todo::fields().user())
        .include(Todo::fields().user().todos());

    let _batched = toasty::batch((User::filter_by_name("Alice"), Todo::all()));
}

#[allow(unused_variables)]
async fn modeling_and_querying_basics_snippets_compile(mut db: toasty::Db) -> toasty::Result<()> {
    let mut user = User::create()
        .name("hello")
        .nickname("h")
        .exec(&mut db)
        .await?;

    user.update().name("hello again").exec(&mut db).await?;

    let user = User::create()
        .name("Alice")
        .nickname("Ali")
        .exec(&mut db)
        .await?;

    let user = User::get_by_id(&mut db, &user.id).await?;

    let users = User::filter_by_name("Alice")
        .collect::<Vec<_>>(&mut db)
        .await?;

    let named_users = User::filter(
        User::fields()
            .name()
            .eq("Alice")
            .or(User::fields().nickname().eq("Ali")),
    )
    .collect::<Vec<_>>(&mut db)
    .await?;

    let page: toasty::Page<Todo> = Todo::all()
        .order_by(Todo::fields().title().asc())
        .paginate(20)
        .collect(&mut db)
        .await?;

    let top_five: Vec<Todo> = Todo::all()
        .order_by(Todo::fields().title().desc())
        .limit(5)
        .collect(&mut db)
        .await?;

    Ok(())
}

#[allow(unused_variables)]
async fn relationships_transactions_and_batch_snippets_compile(
    mut db: toasty::Db,
    user_id: uuid::Uuid,
) -> toasty::Result<()> {
    let user = User::create()
        .name("Ann")
        .nickname("ann")
        .exec(&mut db)
        .await?;

    let todo = user
        .todos()
        .create()
        .title("write docs")
        .exec(&mut db)
        .await?;

    let owner = todo.user().get(&mut db).await?;
    user.todos().insert(&mut db, &todo).await?;
    let same_todo = user.todos().get_by_id(&mut db, &todo.id).await?;
    user.todos().remove(&mut db, &todo).await?;

    let user = User::filter_by_id(user_id)
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    let todos = Todo::all()
        .include(Todo::fields().user())
        .include(Todo::fields().user().todos())
        .collect::<Vec<_>>(&mut db)
        .await?;

    let user = User::filter_by_id(user_id)
        .include(User::fields().todos().user())
        .get(&mut db)
        .await?;

    let mut tx = db.transaction().await?;
    User::create()
        .name("Alice")
        .nickname("alice")
        .exec(&mut tx)
        .await?;
    tx.commit().await?;

    let (users, todos): (Vec<User>, Vec<Todo>) =
        toasty::batch((User::filter_by_name("Alice"), Todo::all()))
            .exec(&mut db)
            .await?;

    Ok(())
}

#[allow(unused_variables)]
async fn macro_and_composite_key_snippets_compile(mut db: toasty::Db) -> toasty::Result<()> {
    let user = toasty::create!(User, {
        name: "Carl",
        nickname: "carl",
    })
    .exec(&mut db)
    .await?;

    let todo = toasty::create!(user.todos(), { title: "get something done" })
        .exec(&mut db)
        .await?;

    let users = toasty::create!(User, [
        { name: "Alice", nickname: "a" },
        { name: "Bob", nickname: "b" }
    ])
    .exec(&mut db)
    .await?;

    let nested = toasty::create!(User, {
        name: "Carl",
        nickname: "nested",
        todos: [{ title: "first" }, { title: "second" }]
    })
    .exec(&mut db)
    .await?;

    let foos: Vec<_> = Foo::filter_by_one_and_two_batch([
        (&"foo-1".to_string(), &"bar-1".to_string()),
        (&"foo-2".to_string(), &"bar-2".to_string()),
    ])
    .collect(&mut db)
    .await?;

    Ok(())
}
