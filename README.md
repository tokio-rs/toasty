# Toasty

Toasty is an ORM for the Rust programming language that prioritizes ease-of-use.
It currently supports SQL databases (SQLite, PostgreSQL, MySQL) and DynamoDB.
Note that Toasty does not hide database capabilities. Instead, Toasty exposes
features based on the target database.

Current status: Preview — Most major features are in place and Toasty should be
complete enough to build applications with. The API is not yet stable and
breaking changes may still occur. Contributions are welcome.

[User guide](https://tokio-rs.github.io/toasty/nightly/guide/): Explore Toasty in depth.

[Nightly API docs](https://tokio-rs.github.io/toasty/nightly/api/): API reference built from the latest commit.

## Using Toasty

You will define your data model using Rust structs annotated with the
`#[derive(toasty::Model)]` derive macro. Here is the
[hello-toasty](examples/hello-toasty/src/main.rs) example.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}
```

Then, you can easily work with the data model:

```rust
// Create a new user and give them some todos.
let user = User::create()
    .name("John Doe")
    .email("john@example.com")
    .todo(Todo::create().title("Make pizza"))
    .todo(Todo::create().title("Finish Toasty"))
    .todo(Todo::create().title("Sleep"))
    .exec(&mut db)
    .await?;

// Load the user from the database
let user = User::get_by_id(&mut db, &user.id).await?;

// Load and iterate the user's todos
let todos = user.todos().exec(&mut db).await?;

for todo in &todos {
    println!("{:#?}", todo);
}
```

## SQL and NoSQL

Toasty supports both SQL and NoSQL databases. Current drivers are SQLite,
PostgreSQL, MySQL, and DynamoDB. However, it does not aim to abstract the
database. Instead, Toasty leans into the target database's capabilities and
aims to help the user avoid issuing inefficient queries for that database.

When targeting both SQL and NoSQL databases, Toasty generates query methods
(e.g. `get_by_id` only for access patterns that are indexed). When targeting a
SQL database, Toasty might allow arbitrary additional query constraints. When
targeting a NoSQL database, Toasty will only allow constraints that the
specific target database can execute. For example, with DynamoDB, query methods
might be generated based on the table's primary key, and additional constraints
may be set for the sort key.

## Application data model vs. database schema

Toasty decouples the application data model from the database's schema. By
default, a toasty application schema will map 1-1 with a database schema.
However, additional annotations may be specified to customize how the
application data model maps to the database schema.

## Roadmap

Development priorities are based on feedback and contributions. If you run into
missing features or rough edges, please open an issue or submit a pull request.

## License

This project is licensed under the [MIT license].

[MIT license]: LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Toasty by you, shall be licensed as MIT, without any additional
terms or conditions.
