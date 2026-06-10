use toasty::{Deferred, Model};

#[derive(Model)]
#[key(account, sk)]
struct A {
    account: String,
    sk: String,
}

#[derive(Model)]
#[key(account, sk)]
struct B {
    account: String,
    sk: String,
}

#[derive(Model)]
struct Bad {
    account: String,
    sk: String,
    #[item_parent]
    a: Deferred<A>,
    #[item_parent]
    b: Deferred<B>,
}

fn main() {}
