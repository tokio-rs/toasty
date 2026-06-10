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

// `User` is an item-collection child. The create-builder for an IC child
// suppresses setters for:
//   * the partition field (`account`) — inherited from the parent handle (R2.4)
//   * the sort field (`sk`)           — owned by Toasty's encoder (R7.1)
//   * the `#[item_parent]` field      — navigation, not a setter (already
//     suppressed since B4.7)
// All three accesses below must fail to compile.
async fn _set_sort(db: &mut Db, tenant: &Tenant) -> Result<User> {
    tenant.users().create().sk("manual".to_string()).name("Alice".to_string()).exec(db).await
}

async fn _set_partition(db: &mut Db, tenant: &Tenant) -> Result<User> {
    tenant.users().create().account("acme".to_string()).name("Alice".to_string()).exec(db).await
}

fn main() {}
