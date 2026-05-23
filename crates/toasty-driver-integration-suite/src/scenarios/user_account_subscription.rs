use crate::prelude::*;

scenario! {
    //! A 2-step `has_one` via chain: `User` → `Account` → `Subscription`.
    //!
    //! `User::subscription` is the via (`account.subscription`). Every step is
    //! a `has_one`, so the via target is a single record — used to test the
    //! single-result (`query.single`) path of via-include lowering, which the
    //! all-`has_many` `user_org_project_todo` scenario never exercises.

    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_one]
        account: toasty::Deferred<Option<Account>>,

        // User → account → subscription, all single (`has_one`) steps.
        #[has_one(via = account.subscription)]
        subscription: toasty::Deferred<Option<Subscription>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Account {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        user_id: Option<ID>,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,

        #[has_one]
        subscription: toasty::Deferred<Option<Subscription>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Subscription {
        #[key]
        #[auto]
        id: ID,

        #[unique]
        account_id: Option<ID>,

        #[belongs_to(key = account_id, references = id)]
        account: toasty::Deferred<Option<Account>>,

        plan: String,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Account, Subscription)).await
    }
}
