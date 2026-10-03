//! `stmt::patch` may step through embedded structs and may replace an
//! embedded enum as a whole, but it may not enter an enum variant. These
//! tests pin that boundary: a path into a variant rejects the whole update
//! with `unsupported_feature` before any write, while the supported forms
//! on the same model keep working.

use crate::prelude::*;
use toasty::stmt;

#[derive(Debug, PartialEq, toasty::Embed)]
enum State {
    Active { count: i64 },
    Inactive,
}

#[derive(Debug, PartialEq, toasty::Embed)]
enum Nested {
    Wrapped { state: State },
    Empty,
}

/// `state` sits after `unrelated`, so the variant-local index of
/// `Active.count` (0) names `unrelated` when the variant root is dropped.
#[derive(Debug, PartialEq, toasty::Embed)]
struct Details {
    unrelated: i64,
    state: State,
    nested: Nested,
}

#[derive(Debug, toasty::Model)]
struct Item {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    details: Details,
}

fn initial_details() -> Details {
    Details {
        unrelated: 1,
        state: State::Active { count: 5 },
        nested: Nested::Wrapped {
            state: State::Active { count: 7 },
        },
    }
}

async fn setup(t: &mut Test) -> Result<(toasty::Db, Item)> {
    let mut db = t.setup_db(models!(Item)).await;
    let item = toasty::create!(Item {
        name: "before",
        details: initial_details(),
    })
    .exec(&mut db)
    .await?;
    Ok((db, item))
}

/// Executes an update carrying `patch` on `details` beside an ordinary
/// assignment to `name`, and checks that the statement is rejected as a
/// whole: `unsupported_feature`, no driver operation, and every field of
/// the row unchanged.
async fn assert_rejected(
    t: &Test,
    db: &mut toasty::Db,
    item: &Item,
    patch: stmt::Assignment<Details>,
) -> Result<()> {
    t.log().clear();
    let err = assert_err!(
        Item::filter_by_id(item.id)
            .update()
            .name("after")
            .details(patch)
            .exec(db)
            .await
    );
    assert!(err.is_unsupported_feature(), "{err}");
    assert!(t.log().is_empty());

    let found = Item::get_by_id(db, item.id).await?;
    assert_eq!(found.name, "before");
    assert_eq!(found.details, initial_details());
    Ok(())
}

#[driver_test]
pub async fn reject_set_through_variant(t: &mut Test) -> Result<()> {
    let (mut db, item) = setup(t).await?;
    let patch = stmt::patch(Details::fields().state().active().count(), 9);
    assert_rejected(t, &mut db, &item, patch).await
}

#[driver_test]
pub async fn reject_increment_through_variant(t: &mut Test) -> Result<()> {
    let (mut db, item) = setup(t).await?;
    let patch = stmt::patch(
        Details::fields().state().active().count(),
        stmt::increment(),
    );
    assert_rejected(t, &mut db, &item, patch).await
}

#[driver_test]
pub async fn reject_patch_through_nested_variants(t: &mut Test) -> Result<()> {
    let (mut db, item) = setup(t).await?;
    let count = || {
        Details::fields()
            .nested()
            .wrapped()
            .state()
            .active()
            .count()
    };

    assert_rejected(t, &mut db, &item, stmt::patch(count(), 9)).await?;
    assert_rejected(t, &mut db, &item, stmt::patch(count(), stmt::increment())).await
}

/// The supported forms on the same model: patching a struct field beside
/// the enum, and replacing an enum nested in the struct as a whole.
#[driver_test]
pub async fn patch_beside_variant_boundary(t: &mut Test) -> Result<()> {
    let (mut db, mut item) = setup(t).await?;

    item.update()
        .details(stmt::apply([
            stmt::patch(Details::fields().unrelated(), stmt::increment()),
            stmt::patch(Details::fields().state().into(), State::Inactive),
        ]))
        .exec(&mut db)
        .await?;
    assert_eq!(
        Item::get_by_id(&mut db, item.id).await?.details,
        Details {
            unrelated: 2,
            state: State::Inactive,
            nested: Nested::Wrapped {
                state: State::Active { count: 7 },
            },
        }
    );

    item.update()
        .details(stmt::patch(
            Details::fields().nested().into(),
            Nested::Wrapped {
                state: State::Active { count: 8 },
            },
        ))
        .exec(&mut db)
        .await?;
    assert_eq!(
        Item::get_by_id(&mut db, item.id).await?.details,
        Details {
            unrelated: 2,
            state: State::Inactive,
            nested: Nested::Wrapped {
                state: State::Active { count: 8 },
            },
        }
    );

    Ok(())
}
