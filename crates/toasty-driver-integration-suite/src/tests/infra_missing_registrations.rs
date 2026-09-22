use crate::prelude::*;

/// Registering only the Parent model should auto-discover the Child model
/// through the BelongsTo relation.
#[driver_test]
pub async fn missing_registration_belongs_to(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,

        child_id: uuid::Uuid,
        #[belongs_to(key = child_id, references = id)]
        child: toasty::Deferred<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    // Auto-discovery should find Child through the BelongsTo relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Child model
/// through the HasOne relation.
#[driver_test]
pub async fn missing_registration_has_one(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_one]
        child: toasty::Deferred<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        parent_id: uuid::Uuid,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::Deferred<Parent>,
    }

    // Auto-discovery should find Child through the HasOne relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Child model
/// through the HasMany relation.
#[driver_test]
pub async fn missing_registration_has_many(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        children: toasty::Deferred<Vec<Child>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[index]
        parent_id: uuid::Uuid,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::Deferred<Parent>,
    }

    // Auto-discovery should find Child through the HasMany relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Detail embedded
/// model through the embedded field.
#[driver_test]
pub async fn missing_registration_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,

        detail: Detail,
    }

    #[derive(Debug, toasty::Embed)]
    struct Detail {
        x: i32,
    }

    // Auto-discovery should find Detail through the embedded field.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}
