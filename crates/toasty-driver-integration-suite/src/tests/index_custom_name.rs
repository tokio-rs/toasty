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

/// Auto-generated index names that exceed the backend's identifier limit are
/// truncated and given a stable 5-character hash suffix (`_XXXX`). The table
/// can still be created and the index is usable for queries.
///
/// The bare auto-generated name for this model is:
/// `index_organization_memberships_by_organization_id_and_member_user_id`
/// (69 chars), which exceeds MySQL's 64-char and PostgreSQL's 63-char limits.
/// With the test harness table prefix it is longer still.
#[driver_test]
pub async fn index_long_name_is_truncated(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[index(organization_id, member_user_id)]
    struct OrganizationMembership {
        // String key avoids auto-increment, which DynamoDB does not support.
        #[key]
        id: String,
        organization_id: i64,
        member_user_id: i64,
    }

    let mut db = t.setup_db(models!(OrganizationMembership)).await;

    let limit = db.capability().max_identifier_length;
    let table = &db.schema().db.tables[0];
    let auto_idx = table
        .indices
        .iter()
        .find(|i| !i.primary_key)
        .expect("non-PK index should exist");

    if let Some(limit) = limit {
        assert!(
            auto_idx.name.len() <= limit,
            "index name `{}` ({} chars) exceeds limit {}",
            auto_idx.name,
            auto_idx.name.len(),
            limit
        );
    }

    // The index must be usable regardless of name length.
    toasty::create!(OrganizationMembership::[
        { id: "m1", organization_id: 1_i64, member_user_id: 10_i64 },
        { id: "m2", organization_id: 1_i64, member_user_id: 20_i64 },
        { id: "m3", organization_id: 2_i64, member_user_id: 10_i64 },
    ])
    .exec(&mut db)
    .await?;

    let members: Vec<OrganizationMembership> =
        OrganizationMembership::filter_by_organization_id(1_i64)
            .exec(&mut db)
            .await?;
    assert_eq!(members.len(), 2);

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
