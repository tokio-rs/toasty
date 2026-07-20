use crate::prelude::*;

scenario! {
    //! A bidirectional many-to-many relation implemented with a join model.
    //!
    //! `User::groups` traverses `Membership::group`, while `Group::users`
    //! traverses the same rows through `Membership::user`. The composite key
    //! permits only one membership for each user-group pair. `role` represents
    //! data stored on the relation itself.

    #![id(ID)]

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        memberships: toasty::Deferred<Vec<Membership>>,

        #[has_many(via = memberships.group)]
        groups: toasty::Deferred<Vec<Group>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Group {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[has_many]
        memberships: toasty::Deferred<Vec<Membership>>,

        #[has_many(via = memberships.user)]
        users: toasty::Deferred<Vec<User>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(user_id, group_id)]
    struct Membership {
        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<User>,

        #[index]
        group_id: ID,

        #[belongs_to(key = group_id, references = id)]
        group: toasty::Deferred<Group>,

        role: String,
    }

    struct Fixture {
        alice: User,
        carol: User,
        rust: Group,
        empty: Group,
    }

    async fn setup(test: &mut Test) -> toasty::Db {
        test.setup_db(models!(User, Group, Membership)).await
    }

    async fn seed(db: &mut toasty::Db) -> Result<Fixture> {
        let mut users = toasty::create!(User::[
            { name: "Alice" },
            { name: "Bob" },
            { name: "Carol" },
        ])
        .exec(&mut *db)
        .await?;
        let alice = users.remove(0);
        let bob = users.remove(0);
        let carol = users.remove(0);

        let mut groups = toasty::create!(Group::[
            { name: "Rust" },
            { name: "Databases" },
            { name: "Empty" },
        ])
        .exec(&mut *db)
        .await?;
        let rust = groups.remove(0);
        let databases = groups.remove(0);
        let empty = groups.remove(0);

        toasty::create!(Membership::[
            { user: &alice, group: &rust,      role: "owner"  },
            { user: &alice, group: &databases, role: "member" },
            { user: &bob,   group: &rust,      role: "member" },
        ])
        .exec(&mut *db)
        .await?;

        Ok(Fixture {
            alice,
            carol,
            rust,
            empty,
        })
    }
}
