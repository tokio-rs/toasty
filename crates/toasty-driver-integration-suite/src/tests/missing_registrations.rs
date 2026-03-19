use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn missing_registration_belongs_to(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,

        child_id: ID,
        #[belongs_to(key = child_id, references = id)]
        child: toasty::BelongsTo<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: ID,
    }

    let error = t.try_setup_db(models!(Parent)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Parent::child"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_has_one(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        child: toasty::HasOne<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: ID,

        parent_id: ID,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::BelongsTo<Parent>,
    }

    let error = t.try_setup_db(models!(Parent)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Parent::child"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_has_many(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        children: toasty::HasMany<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: ID,

        #[index]
        parent_id: ID,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::BelongsTo<Parent>,
    }

    let error = t.try_setup_db(models!(Parent)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Parent::child"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,

        detail: Detail,
    }

    #[derive(Debug, toasty::Embed)]
    struct Detail {
        x: i32,
    }

    let error = t.try_setup_db(models!(Parent)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Parent::detail"));

    Ok(())
}
