use crate::prelude::*;

// A bare `#[belongs_to]` infers `key` from the field name (`user` -> `user_id`)
// and `references` from the target's primary key.
#[driver_test(id(ID))]
pub async fn infers_key_and_references(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        posts: toasty::Deferred<Vec<Post>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: ID,

        #[belongs_to]
        user: toasty::Deferred<User>,

        title: String,
    }

    let mut db = test.setup_db(models!(User, Post)).await;

    let user = User::create().exec(&mut db).await?;
    let post = user.posts().create().title("hello").exec(&mut db).await?;

    assert_eq!(post.user_id, user.id);

    let reloaded = post.user().exec(&mut db).await?;
    assert_eq!(reloaded.id, user.id);

    Ok(())
}

// A nullable `#[belongs_to]` also infers `key` and `references`.
#[driver_test(id(ID))]
pub async fn infers_optional_relation(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        user_id: Option<ID>,

        #[belongs_to]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Post)).await;

    let orphan = toasty::create!(Post {}).exec(&mut db).await?;
    assert_eq!(orphan.user_id, None);

    let user = User::create().exec(&mut db).await?;
    let post = toasty::create!(Post { user: &user }).exec(&mut db).await?;
    assert_eq!(post.user_id, Some(user.id));

    let loaded = post.user().exec(&mut db).await?;
    assert_struct!(loaded, Some(_ { id: == user.id }));

    Ok(())
}

// An explicit `key` with `references` omitted still infers the target's primary
// key.
#[driver_test(id(ID))]
pub async fn infers_references_only(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        #[index]
        owner_id: ID,

        #[belongs_to(key = owner_id)]
        owner: toasty::Deferred<User>,
    }

    let mut db = test.setup_db(models!(User, Post)).await;

    let user = User::create().exec(&mut db).await?;
    let post = toasty::create!(Post { owner: &user }).exec(&mut db).await?;

    assert_eq!(post.owner_id, user.id);

    let reloaded = post.owner().exec(&mut db).await?;
    assert_eq!(reloaded.id, user.id);

    Ok(())
}
