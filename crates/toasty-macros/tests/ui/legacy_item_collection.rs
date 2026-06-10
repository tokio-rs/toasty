use toasty::Model;

#[derive(Model)]
#[key(account, sk)]
struct Tenant {
    account: String,
    sk: String,
}

#[derive(Model)]
#[item_collection(Tenant)]
struct User {
    account: String,
    sk: String,
}

fn main() {}
