use toasty::Model;

#[derive(Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    sk: String,
}

#[derive(Model)]
struct User {
    account: String,
    sk: String,
    #[item_parent]
    tenant: Tenant, // ERROR: must be Deferred<Tenant>
}

fn main() {}
