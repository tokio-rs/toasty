use crate::prelude::*;

/// `#[index(name = "...", ...)]` overrides the auto-generated index name
/// in the DB schema, and the index is still usable for queries.
#[driver_test]
pub async fn index_custom_name_overrides_default(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[index(name = "tournament_region_idx", tournament_id, region)]
    struct Match {
        #[key]
        id: String,
        tournament_id: String,
        region: String,
    }

    let mut db = t.setup_db(models!(Match)).await;

    // Schema carries the user-provided name (not the auto-generated form).
    let table = &db.schema().db.tables[0];
    let custom_idx = table
        .indices
        .iter()
        .find(|i| !i.primary_key)
        .expect("non-PK index should exist");
    assert_eq!(custom_idx.name, "tournament_region_idx");

    // The auto-generated form must NOT be present.
    assert!(
        !table
            .indices
            .iter()
            .any(|i| i.name == "index_matches_by_tournament_id_and_region"),
        "auto-generated name should not coexist with the custom name"
    );

    toasty::create!(Match::[
        { id: "m1", tournament_id: "WINTER2024", region: "NA-EAST" },
        { id: "m2", tournament_id: "WINTER2024", region: "EU-WEST" },
    ])
    .exec(&mut db)
    .await?;

    let matches: Vec<Match> = Match::filter_by_tournament_id("WINTER2024")
        .exec(&mut db)
        .await?;
    assert_eq!(matches.len(), 2);

    Ok(())
}

/// Without `name = "..."`, the schema builder still produces the
/// auto-generated `index_<table>_by_<cols>` form. Sanity check that the
/// custom-name path doesn't accidentally suppress all auto-naming.
#[driver_test]
pub async fn index_custom_name_default_unchanged(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[index(category)]
    struct Product {
        #[key]
        id: String,
        category: String,
    }

    let db = t.setup_db(models!(Product)).await;
    let table = &db.schema().db.tables[0];

    let auto_idx = table
        .indices
        .iter()
        .find(|i| !i.primary_key)
        .expect("non-PK index should exist");
    // Suite prefixes the table name; assert the structural form, not the literal.
    assert!(
        auto_idx.name.starts_with("index_") && auto_idx.name.ends_with("_by_category"),
        "expected auto-generated `index_<table>_by_category`, got: {}",
        auto_idx.name
    );

    Ok(())
}

/// `#[key(name = "...", ...)]` records the custom name on the primary-key
/// index in the DB schema. SQL backends emit primary keys inline today, so
/// this verifies the schema-internal wiring rather than DDL output.
#[driver_test]
pub async fn key_custom_name_recorded_on_pk_index(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(name = "player_pk", partition = team, local = name)]
    struct Player {
        team: String,
        name: String,
    }

    let db = t.setup_db(models!(Player)).await;
    let table = &db.schema().db.tables[0];

    let pk_index = table
        .indices
        .iter()
        .find(|i| i.primary_key)
        .expect("PK index should exist");
    assert_eq!(pk_index.name, "player_pk");

    Ok(())
}
