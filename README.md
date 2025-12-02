# Toasty

**Current status: Incubating - Toasty is not ready for production usage. The API
is still evolving and documentation is lacking.**

Toasty is an ORM for the Rust programming language that prioritizes ease-of-use.
It supports both SQL databases as well as some NoSQL databases, including DynamoDB
and Cassandra. Note that Toasty does not hide the database capabilities.
Instead, Toasty exposes features based on the target database.

## Using Toasty

You will define your data model using Rust structs annotated with the
`#[toasty::model]` procedural macro. Here is the
[hello-toasty](examples/hello-toasty/src/main.rs) example.

```rust
#[derive(Debug, Model)]
struct User {
    #[key]
    #[auto]
    id: Id<Self>,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,

    moto: Option<String>,
}

#[derive(Debug, Model)]
struct Todo {
    #[key]
    #[auto]
    id: Id<Self>,

    #[index]
    user_id: Id<User>,

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
    .exec(&db)
    .await?;

// Load the user from the database
let user = User::get_by_id(&db, &user.id).await?

// Load and iterate the user's todos
let mut todos = user.todos().all(&db).await.unwrap();

while let Some(todo) = todos.next().await {
    let todo = todo.unwrap();
    println!("{:#?}", todo);
}
```

## SQL and NoSQL

Toasty supports both SQL and NoSQL databases, including Cassandra and DynamoDB.
However, it does not aim to abstract the database. Instead, Toasty leans into
the target database's capabilities and aims to help the user avoid issuing
innefficient queries for that database.

When targetting both SQL and NoSQL databases, Toasty generates query methods
(e.g. `find_by_id` only for access patterns that are indexed). When targetting a
SQL database, Toasty might allow arbitrary additional query constraints. When
targetting a NoSQL database, Toasty will only allow constraints that the
specific target database can execute. For example, with DynamoDB, query methods
might be generated based on the table's primary key, and additional constraints
may be set for the sort key.

## Application data model vs. database schema

Toasty decouples the application datamodel from the database's schema. By
default, a toasty application schema will map 1-1 with a database schema.
However, additional annotations may be specified to customize how the
application data model maps to the database schema.

## Current status and roadmap

Toasty is still in the early development stages and is considered
**incubating**. There is no commitment to on-going maintenance or development.
At some point in the future, as the project evolves, this may change. As such,
we encourage you to explore, experiment, and contribute to Toasty, but do not
try using it in production.

Immediate next steps for the project are to fill obvious gaps, such as implement
error handling, remove panics throughout the code base, support additional data
types, and write documentation. After that, development will be based on
feedback and contribution.

## License

This project is licensed under the [MIT license].

[MIT license]: LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Toasty by you, shall be licensed as MIT, without any additional
terms or conditions.
