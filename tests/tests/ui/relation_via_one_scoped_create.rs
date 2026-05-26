#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,

    #[has_one]
    account: toasty::Deferred<Option<Account>>,

    #[has_one(via = account.subscription)]
    subscription: toasty::Deferred<Option<Subscription>>,
}

#[derive(Debug, toasty::Model)]
struct Account {
    #[key]
    id: i64,

    user_id: Option<i64>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<Option<User>>,

    #[has_one]
    subscription: toasty::Deferred<Option<Subscription>>,
}

#[derive(Debug, toasty::Model)]
struct Subscription {
    #[key]
    id: i64,

    account_id: Option<i64>,

    #[belongs_to(key = account_id, references = id)]
    account: toasty::Deferred<Option<Account>>,

    plan: String,
}

fn main() {
    let user = User {
        id: 1,
        account: Default::default(),
        subscription: Default::default(),
    };

    let _ = user.subscription().create().plan("pro");
}
