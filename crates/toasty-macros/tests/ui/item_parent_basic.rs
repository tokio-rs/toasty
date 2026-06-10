use toasty::{Deferred, Model};

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

async fn _navigate(db: &mut toasty::Db, user: User) -> toasty::Result<Tenant> {
    // `#[item_parent]` synthesises a `BelongsTo` relation, so the parent
    // resolves through the same `obj.field().exec(...)` navigation as a
    // hand-written `#[belongs_to(...)]`.
    user.tenant().exec(db).await
}

fn main() {}
