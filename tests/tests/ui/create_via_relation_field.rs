#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,

    name: String,

    #[has_many]
    todos: toasty::Deferred<Vec<Todo>>,

    #[has_many(via = todos.tags)]
    tags: toasty::Deferred<Vec<Tag>>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    id: i64,

    user_id: i64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    #[has_many]
    tags: toasty::Deferred<Vec<Tag>>,
}

#[derive(Debug, toasty::Model)]
struct Tag {
    #[key]
    id: i64,

    todo_id: i64,

    #[belongs_to(key = todo_id, references = id)]
    todo: toasty::Deferred<Todo>,

    name: String,
}

fn main() {
    let _ = toasty::create!(User {
        id: 1,
        name: "Alice",
        tags: [{ id: 1, name: "urgent" }],
    });
}
