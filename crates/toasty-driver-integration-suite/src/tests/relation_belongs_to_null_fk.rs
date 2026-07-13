use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn optional_belongs_to_null_fk(test: &mut Test) -> Result<()> {
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

        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = test.setup_db(models!(User, Post)).await;

    let orphan = toasty::create!(Post {}).exec(&mut db).await?;
    assert_eq!(orphan.user_id, None);

    // Should return `None` rather than panic in the planner.
    assert_none!(orphan.user().exec(&mut db).await?);

    Ok(())
}
