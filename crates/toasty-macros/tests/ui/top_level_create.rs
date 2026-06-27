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

fn _bad() {
    // `User` is an item-collection child (declares `#[item_parent]`). Top-level
    // `User::create()` would let the caller provide a partition + sort key
    // independent of the parent, bypassing R2.4 (same-partition guarantee) and
    // R7.1 (hierarchical sk encoding). The macro suppresses the inherent
    // method on IC children; this access must fail to compile.
    let _ = User::create();
}

fn main() {}
