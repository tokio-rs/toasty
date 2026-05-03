//! Round-trip tests for `Vec<scalar>` model fields.
//!
//! Phase 1 of the document-fields rollout: a Vec of scalars is a queryable
//! collection field. This file only checks whole-value insert/read/update —
//! query operators (`.contains`, `.len`) and in-place mutations (`stmt::push`,
//! `stmt::clear`) land in subsequent PRs.

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn vec_string_round_trip(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let created = Item::create()
        .tags(vec!["admin".to_string(), "verified".to_string()])
        .exec(&mut db)
        .await?;

    assert_eq!(
        created.tags,
        vec!["admin".to_string(), "verified".to_string()]
    );

    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.tags, vec!["admin".to_string(), "verified".to_string()]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn vec_string_empty_round_trip(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let created = Item::create()
        .tags(Vec::<String>::new())
        .exec(&mut db)
        .await?;
    assert!(created.tags.is_empty());

    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert!(read.tags.is_empty());

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn vec_i64_round_trip(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        scores: Vec<i64>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let created = Item::create().scores(vec![1, 2, 3]).exec(&mut db).await?;
    assert_eq!(created.scores, vec![1, 2, 3]);

    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.scores, vec![1, 2, 3]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn vec_string_full_replace(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut created = Item::create()
        .tags(vec!["a".to_string()])
        .exec(&mut db)
        .await?;

    created
        .update()
        .tags(vec!["b".to_string(), "c".to_string()])
        .exec(&mut db)
        .await?;

    let read = Item::get_by_id(&mut db, &created.id).await?;
    assert_eq!(read.tags, vec!["b".to_string(), "c".to_string()]);

    Ok(())
}
