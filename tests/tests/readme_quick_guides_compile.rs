//! Compile-smoke coverage for README mini-guide snippets.
//!
//! These tests validate that the documented API surfaces in README continue to
//! compile.

#![allow(dead_code)]

use toasty::Executor;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[unique]
    email: String,

    #[index]
    group: String,

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

    title: String,
}

#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Meta {
    tags: Vec<String>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
    address: Address,
    #[serialize(json)]
    meta: Meta,
}

#[test]
fn using_toasty_snippet_compiles() {
    let _user_create = User::create()
        .name("John Doe")
        .email("john@example.com")
        .todo(Todo::create().title("Make pizza"))
        .todo(Todo::create().title("Finish Toasty"))
        .todo(Todo::create().title("Sleep"));

    let _todos_query = User::all().include(User::fields().todos());
}

#[test]
fn querying_and_filtering_snippet_compiles() {
    let _john_query = User::filter_by_email("john@example.com");

    let _users_query = User::filter(
        User::fields()
            .name()
            .eq("John Doe")
            .or(User::fields().name().eq("Jane Doe")),
    )
    .order_by(User::fields().name().asc())
    .limit(20);
}

#[test]
fn eager_loading_and_pagination_snippets_compile() {
    let _with_todos = User::all().include(User::fields().todos());

    let _page_query = Todo::all()
        .order_by(Todo::fields().title().asc())
        .paginate(10);
}

#[test]
fn batch_and_macro_snippets_compile() {
    let _batched = toasty::batch((
        User::filter(User::fields().name().eq("John Doe")),
        Todo::all(),
    ));

    let _batch_create = Todo::create_many()
        .item(Todo::create().title("one"))
        .item(Todo::create().title("two"));

    let _macro_create = toasty::create!(User, {
        name: "Carl",
        email: "carl@example.com",
        todos: [{ title: "first" }, { title: "second" }]
    });
}

#[test]
fn embedded_and_serialize_snippet_compiles() {
    let _profile_create = Profile::create()
        .address(Address {
            street: "123 Main".to_string(),
            city: "Seattle".to_string(),
        })
        .meta(Meta {
            tags: vec!["docs".to_string()],
        });
}

#[allow(unused_variables)]
async fn transaction_and_reset_snippets_compile(mut db: toasty::Db) -> toasty::Result<()> {
    let mut tx = db.transaction().await?;

    User::create()
        .name("Alice")
        .email("alice@example.com")
        .exec(&mut tx)
        .await?;
    User::create()
        .name("Bob")
        .email("bob@example.com")
        .exec(&mut tx)
        .await?;

    tx.commit().await?;

    db.reset_db().await?;
    db.push_schema().await?;

    Ok(())
}

#[allow(unused_variables)]
async fn load_by_id_snippet_compiles(mut db: toasty::Db, id: uuid::Uuid) -> toasty::Result<()> {
    let user = User::get_by_id(&mut db, id).await?;
    Ok(())
}
