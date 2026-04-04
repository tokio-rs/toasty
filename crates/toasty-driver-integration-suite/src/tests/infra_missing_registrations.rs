//! Tests that models referenced by registered models are automatically
//! discovered and registered via the global inventory.

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn auto_register_belongs_to(t: &mut Test) -> Result<()> {
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

    // Only register Parent — Child should be auto-discovered
    let _db = t.setup_db(models!(Parent)).await;

    Ok(())
}

#[driver_test(id(ID))]
pub async fn auto_register_has_one(t: &mut Test) -> Result<()> {
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

        #[unique]
        parent_id: ID,
        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::BelongsTo<Parent>,
    }

    // Only register Parent — Child should be auto-discovered
    let _db = t.setup_db(models!(Parent)).await;

    Ok(())
}

#[driver_test(id(ID))]
pub async fn auto_register_has_many(t: &mut Test) -> Result<()> {
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

    // Only register Parent — Child should be auto-discovered
    let _db = t.setup_db(models!(Parent)).await;

    Ok(())
}

#[driver_test(id(ID))]
pub async fn auto_register_embedded(t: &mut Test) -> Result<()> {
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

    // Only register Parent — Detail should be auto-discovered
    let _db = t.setup_db(models!(Parent)).await;

    Ok(())
}
