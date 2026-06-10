use toasty::{Db, Deferred, Model, Result};

#[derive(Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    sk: String,
    name: String,
    #[has_many]
    users: Deferred<Vec<User>>,
}

#[derive(Model)]
#[key(account, sk)]
struct User {
    account: String,
    sk: String,
    name: String,
    #[item_parent]
    tenant: Deferred<Tenant>,
}

// `User` is an item-collection child. The `create!` macro form must reject
// assignment to Toasty-owned fields the same way the create-builder does
// (B-corr-2):
//   * partition (`account`) — inherited from parent (R2.4)
//   * sort (`sk`)           — owned by encoder (R7.1)
async fn _scoped_sk_assignment(db: &mut Db, tenant: &Tenant) -> Result<User> {
    toasty::create!(in tenant.users() {
        sk: "manual".to_string(),
        name: "Alice".to_string(),
    })
    .exec(db)
    .await
}

async fn _scoped_partition_assignment(db: &mut Db, tenant: &Tenant) -> Result<User> {
    toasty::create!(in tenant.users() {
        account: "acme".to_string(),
        name: "Alice".to_string(),
    })
    .exec(db)
    .await
}

fn main() {}
