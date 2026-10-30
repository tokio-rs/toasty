use toasty::Deferred;

type OptionalAccount = Option<Account>;
type DeferredOptionalAccount = Deferred<OptionalAccount>;
type RequiredSubscription = Subscription;
type DeferredSubscription = Deferred<RequiredSubscription>;
type OptionalSubscription = Option<Subscription>;
type DeferredOptionalSubscription = Deferred<OptionalSubscription>;

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: i64,
    account_id: i64,
    optional_account_id: Option<i64>,
    #[belongs_to(key = account_id)]
    account: Deferred<Account>,
    #[belongs_to(key = optional_account_id)]
    eager_optional: OptionalAccount,
    #[belongs_to(key = optional_account_id)]
    deferred_optional: DeferredOptionalAccount,
    #[has_one(via = eager_optional.subscription)]
    required_eager_intermediate: Subscription,
    #[has_one(via = deferred_optional.subscription)]
    required_deferred_intermediate: Deferred<Subscription>,
    #[has_one(via = account.eager_optional)]
    required_eager_terminal: RequiredSubscription,
    #[has_one(via = account.deferred_optional)]
    required_deferred_terminal: DeferredSubscription,
    #[has_one(via = account.deferred_optional)]
    optional_via: DeferredOptionalSubscription,
    #[has_one(via = optional_via)]
    required_nested: Subscription,
}

#[derive(Debug, toasty::Model)]
struct Account {
    #[key]
    id: i64,
    subscription_id: i64,
    optional_subscription_id: Option<i64>,
    #[belongs_to(key = subscription_id)]
    subscription: Deferred<Subscription>,
    #[belongs_to(key = optional_subscription_id)]
    eager_optional: OptionalSubscription,
    #[belongs_to(key = optional_subscription_id)]
    deferred_optional: DeferredOptionalSubscription,
}

#[derive(Debug, toasty::Model)]
struct Subscription {
    #[key]
    id: i64,
}

fn main() {}
