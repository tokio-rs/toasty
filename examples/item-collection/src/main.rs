//! Three-tier item collection: Tenant → User → Todo.
//!
//! Every model in the collection declares the same two key fields the
//! root names — `account` (partition) and `sk` (sort) — and tags them
//! with `#[key(account, sk)]`. Toasty owns `sk`'s contents and mints a
//! UUID v7 for each row's local-id segment.
//!
//! NOTE: end-to-end `cargo run` requires Task B4 (root sk auto-mint at
//! create time). Until B4 lands, this example compiles but the create
//! call will require `sk` to be supplied explicitly. Use B4 to remove
//! the workaround.
//!
//! Run against local DDB:
//! ```bash
//!  AWS_ENDPOINT_URL_DYNAMODB=http://localhost:8000 \
//!   AWS_ACCESS_KEY_ID=test AWS_SECRET_ACCESS_KEY=test \
//!   AWS_REGION=us-east-1 \
//!   TOASTY_CONNECTION_URL="dynamodb://us-east-1" \
//!   cargo run -p example-item-collection
//! ```

use toasty::{Db, Deferred, Result};

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    sk: String,
    name: String,
    #[has_many]
    users: Deferred<Vec<User>>,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct User {
    account: String,
    sk: String,
    name: String,
    #[item_parent]
    tenant: Deferred<Tenant>,
    #[has_many]
    todos: Deferred<Vec<Todo>>,
}

#[derive(Debug, toasty::Model)]
#[key(account, sk)]
struct Todo {
    account: String,
    sk: String,
    title: String,
    #[item_parent]
    user: Deferred<User>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut db = Db::builder()
        .models(toasty::models!(Tenant, User, Todo))
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    db.push_schema().await?;

    // TODO(B4): remove `sk` after root-sk auto-mint lands.
    let acme = toasty::create!(Tenant {
        account: "acme",
        sk: "Tenant#",
        name: "Acme"
    })
    .exec(&mut db)
    .await?;
    println!("created tenant; account={}", acme.account);

    let alice = acme.users().create().name("Alice").exec(&mut db).await?;
    let bob = acme.users().create().name("Bob").exec(&mut db).await?;
    println!("created users alice.sk={} bob.sk={}", alice.sk, bob.sk);

    populate_todos(&mut db, &alice).await?;
    populate_todos(&mut db, &bob).await?;

    println!("--- Acme's users ---");
    for user in acme.users().exec(&mut db).await? {
        println!("user name={} sk={}", user.name, user.sk);
    }

    println!("--- Alice's todos ---");
    for todo in alice.todos().exec(&mut db).await? {
        println!("todo title={} sk={}", todo.title, todo.sk);
    }

    Ok(())
}

async fn populate_todos(db: &mut Db, user: &User) -> Result<()> {
    let mut builder = Todo::create_many();
    for i in 0..5 {
        builder = builder.item(toasty::create!(in user.todos() {
            title: format!("Todo {} for {}", i, user.name),
        }));
    }
    builder.exec(db).await?;
    Ok(())
}
