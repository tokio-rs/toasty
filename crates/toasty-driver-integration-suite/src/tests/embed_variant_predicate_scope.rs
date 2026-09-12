//! Boolean scope of the variant checks a predicate over an enum variant's
//! field requires: the check is part of the predicate, so negating or
//! combining the predicate treats the guarded comparison as a whole.

use crate::prelude::*;
use toasty::stmt::IntoExpr;

#[driver_test(requires(scan))]
pub async fn optional_field_negation(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    enum State {
        Selected {
            #[shared(value)]
            value: Option<String>,
        },
        Other {
            #[shared(value)]
            value: Option<String>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        state: State,
    }

    let mut db = t.setup_db(models!(Item)).await;
    for value in [None, Some("value".to_string())] {
        toasty::create!(Item {
            state: State::Selected {
                value: value.clone()
            }
        })
        .exec(&mut db)
        .await?;
        toasty::create!(Item {
            state: State::Other { value }
        })
        .exec(&mut db)
        .await?;
    }

    // Four rows: Selected/Other with and without a value. A predicate on
    // `selected().value()` requires the Selected variant; its negation
    // matches every other row, including the Other rows sharing the column.
    let value = || Item::fields().state().selected().value();
    for (predicate, expected) in [
        (value().is_none(), 1),
        (value().is_some(), 1),
        (value().is_none().not(), 3),
        (value().is_some().not(), 3),
        (value().into_expr().is_none().not(), 3),
        (value().into_expr().is_some(), 1),
        (value().is_none().eq(false), 3),
        (value().is_none().or(value().is_some()), 2),
        (value().is_none().or(value().is_some()).not(), 2),
    ] {
        assert_eq!(Item::filter(predicate).exec(&mut db).await?.len(), expected);
    }
    Ok(())
}

#[driver_test(requires(scan))]
pub async fn variant_guard_through_relation(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    enum State {
        Selected {
            #[shared(value)]
            value: String,
        },
        Other {
            #[shared(value)]
            value: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        state: State,
    }

    #[derive(Debug, toasty::Model)]
    struct Link {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        item_id: uuid::Uuid,
        #[belongs_to(key = item_id)]
        item: toasty::Deferred<Item>,
    }

    let mut db = t.setup_db(models!(Item, Link)).await;
    for state in [
        State::Selected {
            value: "same".into(),
        },
        State::Other {
            value: "same".into(),
        },
    ] {
        let item = toasty::create!(Item { state }).exec(&mut db).await?;
        toasty::create!(Link { item: &item }).exec(&mut db).await?;
    }

    // The variant check travels through the relation with the predicate:
    // only the link whose item is Selected matches, although both items
    // hold the same value in the shared column.
    let state = || Link::fields().item().state();
    for predicate in [
        state().selected().value().eq("same"),
        state().is_selected(),
        state().selected().matches(|_| true.into_expr()),
    ] {
        assert_eq!(Link::filter(predicate).exec(&mut db).await?.len(), 1);
    }
    Ok(())
}
