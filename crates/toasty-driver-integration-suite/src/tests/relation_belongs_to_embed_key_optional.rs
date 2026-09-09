use crate::prelude::*;

/// Regression test for tokio-rs/toasty#1204: including an optional
/// `belongs_to` whose key is an `Embed` newtype panicked in the query planner.
#[driver_test]
pub async fn include_optional_belongs_to_with_embed_key(t: &mut Test) -> Result<()> {
    #[derive(Clone, Debug, PartialEq, Eq, toasty::Embed)]
    struct Code(String);

    #[derive(Debug, toasty::Model)]
    struct Region {
        #[key]
        code: Code,
    }

    #[derive(Debug, toasty::Model)]
    struct Office {
        #[key]
        id: u64,

        #[index]
        region_code: Option<Code>,

        #[belongs_to(key = region_code, references = code)]
        region: toasty::Deferred<Option<Region>>,
    }

    let mut db = t.setup_db(models!(Region, Office)).await;

    toasty::create!(Region {
        code: Code("north".into()),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Office {
        id: 1,
        region_code: Some(Code("north".into())),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Office { id: 2 }).exec(&mut db).await?;

    let offices = Office::all()
        .include(Office::fields().region())
        .exec(&mut db)
        .await?;

    assert_eq!(offices.len(), 2);

    let matched = offices.iter().find(|o| o.id == 1).unwrap();
    assert_eq!(matched.region.get().as_ref().unwrap().code.0, "north");

    let orphan = offices.iter().find(|o| o.id == 2).unwrap();
    assert_none!(orphan.region.get().as_ref());

    Ok(())
}
