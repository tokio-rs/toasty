use crate::prelude::*;

/// Registering only the Parent model should auto-discover the Child model
/// through the BelongsTo relation.
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

    // Auto-discovery should find Child through the BelongsTo relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Child model
/// through the HasOne relation.
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

        #[index]
        parent_id: ID,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::BelongsTo<Parent>,
    }

    // Auto-discovery should find Child through the HasOne relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Child model
/// through the HasMany relation.
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

    // Auto-discovery should find Child through the HasMany relation.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}

/// Registering only the Parent model should auto-discover the Detail embedded
/// model through the embedded field.
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

    // Auto-discovery should find Detail through the embedded field.
    t.try_setup_db(models!(Parent)).await?;

    Ok(())
}
