//! Three-level item collection: Tenant -> User -> Todo.
//!
//! All three models share a single DynamoDB table. The partition key is
//! `tenant_id`; the sort key is composed by the driver from `__model` plus
//! each model's own local PK fields:
//!
//!   Tenant row: __sk = "Tenant#"
//!   User row:   __sk = "User#<user_id>#"
//!   Todo row:   __sk = "Todo#<user_id>#<todo_id>#"
//!
//! Querying `tenant.users()` becomes `begins_with(__sk, "User#")` and
//! `user.todos()` becomes `begins_with(__sk, "Todo#<user_id>#")` — both
//! single-partition queries with no scan.
//! Assuming local DDB is running on port 8080:
//! ```bash
//!  AWS_ENDPOINT_URL_DYNAMODB=http://localhost:8000 \
//!   AWS_ACCESS_KEY_ID=test \
//!   AWS_SECRET_ACCESS_KEY=test \
//!   AWS_REGION=us-east-1 \
//!   TOASTY_CONNECTION_URL="dynamodb://us-east-1" \
//!   cargo run -p example-item-collection
//! ```

use toasty::Db;
use toasty::Result;
use uuid::Uuid;

#[derive(Debug, toasty::Model)]
struct Tenant {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[has_many]
    users: toasty::HasMany<User>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(Tenant)]
#[key(partition = tenant_id, local = id)]
struct User {
    id: String,

    tenant_id: uuid::Uuid,

    #[belongs_to(key = tenant_id, references = id)]
    tenant: toasty::BelongsTo<Tenant>,

    name: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
#[item_collection(User)]
#[key(partition = tenant_id, local = [user_id, id])]
#[index(tenant_id, user_id)]
struct Todo {
    id: uuid::Uuid,

    tenant_id: uuid::Uuid,
    user_id: String,

    #[belongs_to(key = [tenant_id, user_id], references = [tenant_id, id])]
    user: toasty::BelongsTo<User>,

    title: String,
}

#[tokio::main]
async fn main() -> toasty::Result<()> {
    let mut db = toasty::Db::builder()
        // Order matters: each child model's table mapping is resolved against
        // its parent's, so parents must be registered first.
        .models(toasty::models!(Tenant, User, Todo))
        .connect(
            std::env::var("TOASTY_CONNECTION_URL")
                .as_deref()
                .unwrap_or("sqlite::memory:"),
        )
        .await?;

    db.push_schema().await?;

    let acme = toasty::create!(Tenant { name: "Acme" })
        .exec(&mut db)
        .await?;

    println!("created tenant; name={:?}", acme.name);

    let alice =
        toasty::create!(in acme.users() { name: "Alice", id: format!("{}", Uuid::new_v4()),})
            .exec(&mut db)
            .await?;
    let bob = toasty::create!(in acme.users() { name: "Bob", id: format!("{}", Uuid::new_v4()) })
        .exec(&mut db)
        .await?;

    // tier 2 create_many
    let _users = populate_users(&mut db, &acme).await?;

    println!("created users; alice={:?}, bob={:?}", alice.name, bob.name);

    // Scoped query: every user under Acme.
    // For DynamoDB this is a single Query with `begins_with(__sk, "User#")`.
    println!("====================");
    println!("--- ACME USERS ---");
    println!("====================");

    let users = acme.users().exec(&mut db).await?;
    for user in users {
        println!("USER name={:?}, id={}", user.name, user.id);
    }

    populate_todos(&mut db, &alice).await?;
    populate_todos(&mut db, &bob).await?;

    println!("================");
    println!("--- Alice's todos -----");
    println!("================");
    // The schema build, table layout, and __sk encoding for Todo are all
    // already in place — see the table inspection below.
    // -------------------------------------------------------------------

    let mut users = User::filter_by_tenant_id(acme.id)
        .filter_by_id(&alice.id)
        .include(User::fields().todos())
        .exec(&mut db)
        .await?;
    let from_db = users.pop().expect("Should have found a user");
    assert_eq!(10, from_db.todos.get().len());
    for t in from_db.todos.get() {
        println!("Todo ID={:?}, TITLE: {}", t.id, t.title);
    }
    Ok(())
}

async fn populate_users(db: &mut Db, tenant: &Tenant) -> Result<Vec<User>> {
    let mut builder = User::create_many();
    for name in ["Carol", "Eric", "Frank"] {
        builder = builder.item(toasty::create!(in tenant.users() {
            id: format!("{}", Uuid::new_v4()),
            name: name,
        }));
    }
    builder.exec(db).await
}
async fn populate_todos(db: &mut Db, user: &User) -> Result<Vec<Todo>> {
    let mut many = Todo::create_many();
    for i in 0..10 {
        many = many.item(toasty::create!(in user.todos() {
            id:  Uuid::new_v4(),
            title: format!("Todo {} for {}", i, user.name)
        }))
    }
    many.exec(db).await
}
