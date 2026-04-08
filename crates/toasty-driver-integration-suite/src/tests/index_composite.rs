use crate::prelude::*;
use toasty_core::driver::Operation;

/// Basic composite index: model-level `#[index(field_a, field_b)]` creates a two-column
/// index on SQL and a GSI (hash + range key) on DynamoDB.
#[driver_test]
pub async fn composite_index_basic(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(user_id, game_title)]
    #[index(game_title, top_score)]
    struct GameScore {
        user_id: String,
        game_title: String,
        top_score: i64,
    }

    let mut db = t.setup_db(models!(GameScore)).await;

    toasty::create!(GameScore::[
        { user_id: "u1", game_title: "chess", top_score: 100_i64 },
        { user_id: "u2", game_title: "chess", top_score: 200_i64 },
        { user_id: "u1", game_title: "go", top_score: 50_i64 },
    ])
    .exec(&mut db)
    .await?;

    let mut scores: Vec<GameScore> = GameScore::filter_by_game_title("chess")
        .exec(&mut db)
        .await?;
    scores.sort_by_key(|s| s.top_score);

    assert_eq!(scores.len(), 2);
    assert_eq!(scores[0].top_score, 100);
    assert_eq!(scores[1].top_score, 200);

    Ok(())
}

/// Struct-level `#[index(field)]` is equivalent to field-level `#[index]` (cross-driver).
///
/// Verifies that `filter_by_user_id()` returns the correct records and issues an
/// indexed operation rather than a full scan.
#[driver_test]
pub async fn composite_index_struct_level(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, name)]
    #[index(user_id)]
    struct Post {
        id: String,
        name: String,
        user_id: String,
        title: String,
    }

    let mut db = t.setup_db(models!(Post)).await;

    toasty::create!(Post::[
        { id: "p1", name: "first", user_id: "alice", title: "Hello World" },
        { id: "p2", name: "second", user_id: "alice", title: "Another Post" },
        { id: "p3", name: "third", user_id: "bob", title: "Bob's Post" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    let mut posts: Vec<Post> = Post::filter_by_user_id("alice").exec(&mut db).await?;
    posts.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].title, "Hello World");
    assert_eq!(posts[1].title, "Another Post");

    // Verify that an indexed operation was issued (not a full scan)
    let op = t.log().pop_op();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    Ok(())
}

/// Two-column index generates prefix query methods for each valid column prefix (cross-driver).
///
/// `#[index(game_title, top_score)]` generates:
/// - `filter_by_game_title()` — partition key only
/// - `filter_by_game_title_and_top_score()` — both columns
///
/// Verifies both methods issue an indexed operation.
#[driver_test]
pub async fn composite_index_prefix_queries(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(user_id, game_title)]
    #[index(game_title, top_score)]
    struct GameScore {
        user_id: String,
        game_title: String,
        top_score: i64,
    }

    let mut db = t.setup_db(models!(GameScore)).await;

    toasty::create!(GameScore::[
        { user_id: "u1", game_title: "chess", top_score: 100_i64 },
        { user_id: "u2", game_title: "chess", top_score: 200_i64 },
        { user_id: "u3", game_title: "chess", top_score: 200_i64 },
        { user_id: "u1", game_title: "go", top_score: 50_i64 },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Test prefix query: partition key only
    let scores: Vec<GameScore> = GameScore::filter_by_game_title("chess")
        .exec(&mut db)
        .await?;
    assert_eq!(scores.len(), 3);

    let op = t.log().pop_op();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    t.log().clear();

    // Test full key query: partition + sort key
    let scores: Vec<GameScore> = GameScore::filter_by_game_title_and_top_score("chess", 100)
        .exec(&mut db)
        .await?;
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0].user_id, "u1");

    let op = t.log().pop_op();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    Ok(())
}

/// Multi-attribute partition key: `#[index(partition = a, partition = b, local = c)]`
/// creates a GSI with 2 HASH + 1 RANGE attributes (DDB-only).
///
/// Verifies prefix queries for all valid access patterns.
#[driver_test(requires(not(sql)))]
pub async fn composite_index_multi_hash(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(partition = tournament_id, partition = region, local = round)]
    struct Match {
        id: String,
        tournament_id: String,
        region: String,
        round: String,
        player1_id: String,
        player2_id: String,
    }

    let mut db = t.setup_db(models!(Match)).await;

    toasty::create!(Match::[
        { id: "m1", tournament_id: "WINTER2024", region: "NA-EAST", round: "SEMIFINALS", player1_id: "alice", player2_id: "bob" },
        { id: "m2", tournament_id: "WINTER2024", region: "NA-EAST", round: "FINALS", player1_id: "charlie", player2_id: "dave" },
        { id: "m3", tournament_id: "WINTER2024", region: "EU-WEST", round: "SEMIFINALS", player1_id: "eve", player2_id: "frank" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Query by all partition key attributes (required for DDB GSI access)
    let mut matches: Vec<Match> =
        Match::filter_by_tournament_id_and_region("WINTER2024", "NA-EAST")
            .exec(&mut db)
            .await?;
    matches.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].round, "SEMIFINALS");
    assert_eq!(matches[1].round, "FINALS");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    t.log().clear();

    // Query by partition key + sort key prefix
    let matches: Vec<Match> =
        Match::filter_by_tournament_id_and_region_and_round("WINTER2024", "NA-EAST", "SEMIFINALS")
            .exec(&mut db)
            .await?;

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].player1_id, "alice");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    Ok(())
}

/// Multi-attribute sort key: `#[index(partition = a, local = b, local = c)]`
/// creates a GSI with 1 HASH + 2 RANGE attributes (DDB-only).
///
/// Verifies all three prefix query methods issue indexed operations.
#[driver_test(requires(not(sql)))]
pub async fn composite_index_multi_range(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(partition = player_id, local = match_date, local = round)]
    struct PlayerMatch {
        id: String,
        player_id: String,
        match_date: String,
        round: String,
        opponent_id: String,
        score: String,
    }

    let mut db = t.setup_db(models!(PlayerMatch)).await;

    toasty::create!(PlayerMatch::[
        { id: "pm1", player_id: "101", match_date: "2024-01-18", round: "SEMIFINALS", opponent_id: "102", score: "3-1" },
        { id: "pm2", player_id: "101", match_date: "2024-01-18", round: "FINALS", opponent_id: "103", score: "2-1" },
        { id: "pm3", player_id: "101", match_date: "2024-01-25", round: "SEMIFINALS", opponent_id: "104", score: "3-0" },
        { id: "pm4", player_id: "999", match_date: "2024-01-18", round: "QUARTERFINALS", opponent_id: "101", score: "1-3" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Query by partition key only — all matches for a player
    let matches: Vec<PlayerMatch> = PlayerMatch::filter_by_player_id("101")
        .exec(&mut db)
        .await?;
    assert_eq!(matches.len(), 3);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    t.log().clear();

    // Query by partition key + first sort key — all matches on a specific date
    let matches: Vec<PlayerMatch> =
        PlayerMatch::filter_by_player_id_and_match_date("101", "2024-01-18")
            .exec(&mut db)
            .await?;
    assert_eq!(matches.len(), 2);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    t.log().clear();

    // Query by partition key + both sort keys — specific match
    let matches: Vec<PlayerMatch> = PlayerMatch::filter_by_player_id_and_match_date_and_round(
        "101",
        "2024-01-18",
        "SEMIFINALS",
    )
    .exec(&mut db)
    .await?;
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].opponent_id, "102");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    Ok(())
}

/// Three-column composite index on SQL: `#[index(country, city, zip_code)]` (SQL-only).
///
/// Verifies all three prefix query methods return correct results.
#[driver_test(requires(sql))]
pub async fn composite_index_three_columns(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(country, city, zip_code)]
    struct Address {
        #[auto]
        id: u64,
        country: String,
        city: String,
        zip_code: String,
        street: String,
    }

    let mut db = t.setup_db(models!(Address)).await;

    toasty::create!(Address::[
        { country: "US", city: "Seattle", zip_code: "98101", street: "1st Ave" },
        { country: "US", city: "Seattle", zip_code: "98102", street: "2nd Ave" },
        { country: "US", city: "Portland", zip_code: "97201", street: "Oak St" },
        { country: "CA", city: "Toronto", zip_code: "M5V", street: "King St" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // 1-column prefix: country only
    let addrs: Vec<Address> = Address::filter_by_country("US").exec(&mut db).await?;
    assert_eq!(addrs.len(), 3);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QuerySql(_));

    t.log().clear();

    // 2-column prefix: country + city
    let addrs: Vec<Address> = Address::filter_by_country_and_city("US", "Seattle")
        .exec(&mut db)
        .await?;
    assert_eq!(addrs.len(), 2);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QuerySql(_));

    t.log().clear();

    // 3-column full key: country + city + zip_code
    let addrs: Vec<Address> =
        Address::filter_by_country_and_city_and_zip_code("US", "Seattle", "98101")
            .exec(&mut db)
            .await?;
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].street, "1st Ave");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QuerySql(_));

    Ok(())
}

/// Three-column simple-mode index on DynamoDB: `#[index(country, city, zip_code)]` (DDB-only).
///
/// In simple mode, the first field becomes HASH and the rest become RANGE.
/// Verifies all three prefix query methods issue `QueryPk` and return correct results.
#[driver_test(requires(not(sql)))]
pub async fn composite_index_simple_three_column_ddb(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(country, city, zip_code)]
    struct Address {
        id: String,
        country: String,
        city: String,
        zip_code: String,
        street: String,
    }

    let mut db = t.setup_db(models!(Address)).await;

    toasty::create!(Address::[
        { id: "a1", country: "US", city: "Seattle", zip_code: "98101", street: "1st Ave" },
        { id: "a2", country: "US", city: "Seattle", zip_code: "98102", street: "2nd Ave" },
        { id: "a3", country: "US", city: "Portland", zip_code: "97201", street: "Oak St" },
        { id: "a4", country: "CA", city: "Toronto", zip_code: "M5V", street: "King St" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // 1-column prefix: HASH key only
    let addrs: Vec<Address> = Address::filter_by_country("US").exec(&mut db).await?;
    assert_eq!(addrs.len(), 3);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    t.log().clear();

    // 2-column prefix: HASH + first RANGE key
    let addrs: Vec<Address> = Address::filter_by_country_and_city("US", "Seattle")
        .exec(&mut db)
        .await?;
    assert_eq!(addrs.len(), 2);

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    t.log().clear();

    // 3-column full key: HASH + both RANGE keys
    let addrs: Vec<Address> =
        Address::filter_by_country_and_city_and_zip_code("US", "Seattle", "98101")
            .exec(&mut db)
            .await?;
    assert_eq!(addrs.len(), 1);
    assert_eq!(addrs[0].street, "1st Ave");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    Ok(())
}

/// Multiple indexes on the same model: verifies the query planner selects the correct
/// index when a model defines two `#[index]` attributes (cross-driver).
///
/// A bug in index selection could silently route queries through the wrong index,
/// returning incorrect results.
#[driver_test]
pub async fn composite_index_multiple_indexes(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(category)]
    #[index(brand)]
    struct Product {
        id: String,
        category: String,
        brand: String,
        name: String,
    }

    let mut db = t.setup_db(models!(Product)).await;

    toasty::create!(Product::[
        { id: "p1", category: "electronics", brand: "acme", name: "Widget A" },
        { id: "p2", category: "electronics", brand: "globex", name: "Widget B" },
        { id: "p3", category: "clothing", brand: "acme", name: "Shirt C" },
        { id: "p4", category: "clothing", brand: "initech", name: "Pants D" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Query via the first index (category)
    let mut products: Vec<Product> = Product::filter_by_category("electronics")
        .exec(&mut db)
        .await?;
    products.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(products.len(), 2);
    assert_eq!(products[0].name, "Widget A");
    assert_eq!(products[1].name, "Widget B");

    let op = t.log().pop_op();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    t.log().clear();

    // Query via the second index (brand) — must not use the category index
    let mut products: Vec<Product> = Product::filter_by_brand("acme").exec(&mut db).await?;
    products.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(products.len(), 2);
    assert_eq!(products[0].name, "Widget A");
    assert_eq!(products[1].name, "Shirt C");

    let op = t.log().pop_op();
    if t.capability().sql {
        assert_struct!(op, Operation::QuerySql(_));
    } else {
        assert_struct!(op, Operation::QueryPk(_));
    }

    Ok(())
}

/// Maximum attribute boundary: `#[index(partition = f1..f4, local = f5..f8)]` (DDB-only).
///
/// DynamoDB allows up to 4 HASH + 4 RANGE attributes in a GSI KeySchema.
/// Verifies that `setup_db()` succeeds at the limit and that a query using all
/// 4 partition key attributes returns correct results.
#[driver_test(requires(not(sql)))]
pub async fn composite_index_max_attributes(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(partition = f1, partition = f2, partition = f3, partition = f4, local = f5, local = f6, local = f7, local = f8)]
    struct MaxIndex {
        id: String,
        f1: String,
        f2: String,
        f3: String,
        f4: String,
        f5: String,
        f6: String,
        f7: String,
        f8: String,
        value: String,
    }

    // setup_db must succeed at the 4+4 boundary
    let mut db = t.setup_db(models!(MaxIndex)).await;

    toasty::create!(MaxIndex::[
        { id: "r1", f1: "a1", f2: "b1", f3: "c1", f4: "d1", f5: "e1", f6: "g1", f7: "h1", f8: "i1", value: "found" },
        { id: "r2", f1: "a1", f2: "b1", f3: "c1", f4: "d2", f5: "e1", f6: "g1", f7: "h1", f8: "i1", value: "other" },
    ])
    .exec(&mut db)
    .await?;

    t.log().clear();

    // Query using all 4 partition key attributes (required for DDB multi-attribute HASH key)
    let records: Vec<MaxIndex> =
        MaxIndex::filter_by_f1_and_f2_and_f3_and_f4("a1", "b1", "c1", "d1")
            .exec(&mut db)
            .await?;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].value, "found");

    let op = t.log().pop_op();
    assert_struct!(op, Operation::QueryPk(_));

    Ok(())
}

/// Error condition: more than 4 RANGE columns in simple-mode index (DDB-only).
///
/// `#[index(a, b, c, d, e, f)]` in simple mode produces 1 HASH + 5 RANGE, which
/// exceeds the DynamoDB limit of 4. The driver must return `Err(invalid_schema)`
/// rather than panicking.
#[driver_test(requires(not(sql)))]
pub async fn composite_index_too_many_range_columns(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id)]
    #[index(a, b, c, d, e, f)]
    struct TooManyRange {
        id: String,
        a: String,
        b: String,
        c: String,
        d: String,
        e: String,
        f: String,
    }

    // Do NOT use `?` — capture the error instead of propagating it
    let result = t.try_setup_db(models!(TooManyRange)).await;

    assert!(
        result.is_err(),
        "expected setup_db to fail for 1 HASH + 5 RANGE index"
    );
    let err = result.unwrap_err();
    assert!(
        err.is_invalid_schema(),
        "expected invalid_schema error, got: {err}"
    );

    Ok(())
}

/// Range filter chained onto a composite index partition query (cross-driver).
///
/// `filter_by_game_title("chess")` uses the index to scope by partition key,
/// then `.filter(GameScore::fields().top_score().gt(150))` applies a range
/// condition on the sort key.
#[driver_test]
pub async fn composite_index_sort_key_range_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(user_id, game_title)]
    #[index(game_title, top_score)]
    struct GameScore {
        user_id: String,
        game_title: String,
        top_score: i64,
    }

    let mut db = t.setup_db(models!(GameScore)).await;

    toasty::create!(GameScore::[
        { user_id: "u1", game_title: "chess", top_score: 100_i64 },
        { user_id: "u2", game_title: "chess", top_score: 200_i64 },
        { user_id: "u3", game_title: "chess", top_score: 1500_i64 },
        { user_id: "u4", game_title: "chess", top_score: 50_i64 },
        { user_id: "u1", game_title: "go", top_score: 9999_i64 },
    ])
    .exec(&mut db)
    .await?;

    let mut scores: Vec<GameScore> = GameScore::filter_by_game_title("chess")
        .filter(GameScore::fields().top_score().gt(150))
        .exec(&mut db)
        .await?;
    scores.sort_by_key(|s| s.top_score);

    assert_eq!(scores.len(), 2);
    assert_eq!(scores[0].top_score, 200);
    assert_eq!(scores[1].top_score, 1500);

    // go scores must not appear despite having top_score > 150
    assert!(scores.iter().all(|s| s.game_title == "chess"));

    Ok(())
}
