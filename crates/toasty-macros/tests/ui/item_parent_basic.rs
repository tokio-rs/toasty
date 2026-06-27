use toasty::{Db, Deferred, Model, Result};

#[derive(Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    sk: String,
    name: String,
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

// B4.8 wired the `child.parent()` accessor for `#[item_parent]` fields.
// `user.tenant().exec(db).await` lowers to a partition-scoped query with a
// `starts_with("Tenant#")` predicate on the sort key (design R2.9). The
// helper exists only to assert the emitted method compiles; the trybuild
// fixture does not run it.
async fn _navigate(user: &User, db: &mut Db) -> Result<Tenant> {
    user.tenant().exec(db).await
}

fn main() {}
