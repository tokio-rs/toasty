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
