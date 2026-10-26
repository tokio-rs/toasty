use toasty::Deferred;

type RequiredProfile = Profile;
type DeferredProfile = Deferred<RequiredProfile>;
type RequiredUser = User;
type DeferredUser = Deferred<RequiredUser>;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
}

#[derive(Debug, toasty::Model)]
struct Account {
    #[key]
    id: i64,
    #[has_one(pair = account)]
    eager: RequiredProfile,
    #[has_one(pair = account)]
    deferred: DeferredProfile,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    id: i64,
    #[unique]
    account_id: i64,
    #[belongs_to(key = account_id)]
    account: Deferred<Account>,
    user_id: i64,
    #[belongs_to(key = user_id)]
    eager: RequiredUser,
    #[belongs_to(key = user_id)]
    deferred: DeferredUser,
}

fn main() {
    Account::fields().eager().is_some();
    Account::fields().eager().is_none();
    Account::fields().deferred().is_some();
    Account::fields().deferred().is_none();
    Profile::fields().eager().is_some();
    Profile::fields().eager().is_none();
    Profile::fields().deferred().is_some();
    Profile::fields().deferred().is_none();
    Account::fields().is_some();
    Account::fields().is_none();
}
