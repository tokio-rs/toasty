//! Test querying models with various filters and constraints

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn query_index_eq(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        email: String,
    }

    let db = test.setup_db(models!(User)).await;

    // Create a few users
    for &(name, email) in &[
        ("one", "one@example.com"),
        ("two", "two@example.com"),
        ("three", "three@example.com"),
    ] {
        User::create()
            .name(name)
            .email(email)
            .exec(&db)
            .await
            .unwrap();
    }

    let users = User::filter_by_name("one")
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();

    assert_eq!(1, users.len());
    assert_eq!("one", users[0].name);

    // Create a second user named "one"

    User::create()
        .name("one")
        .email("one-two@example.com")
        .exec(&db)
        .await
        .unwrap();

    let mut users = User::filter_by_name("one")
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    users.sort_by_key(|u| u.email.clone());

    assert_eq!(2, users.len());
    assert_eq!("one", users[0].name);
    assert_eq!("one-two@example.com", users[0].email);

    assert_eq!("one", users[1].name);
    assert_eq!("one@example.com", users[1].email);
}

#[driver_test(id(ID))]
pub async fn query_partition_key_string_eq(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[key(partition = league, local = name)]
    struct Team {
        league: String,

        name: String,

        founded: i64,
    }

    let db = test.setup_db(models!(Team)).await;

    // Create some teams
    for (league, name, founded) in [
        ("MLS", "Portland Timbers", 2009),
        ("MLS", "Seattle Sounders FC", 2007),
        ("MLS", "Vancouver Whitecaps FC", 2009),
        ("MLS", "Los Angeles Football Club", 2014),
        ("MLS", "San Jose Earthquakes", 1994),
        ("MLS", "LA Galaxy", 1994),
        ("EPL", "Arsenal", 1886),
        ("EPL", "Chelsea", 1905),
        ("EPL", "Manchester United", 1878),
        ("EPL", "Tottenham", 1882),
        ("La Liga", "FC Barcelona", 1899),
        ("La Liga", "Girona FC", 1930),
        ("La Liga", "Real Madrid", 1902),
        ("La Liga", "Atlético Madrid", 1903),
    ]
    .into_iter()
    {
        Team::create()
            .league(league)
            .name(name)
            .founded(founded)
            .exec(&db)
            .await
            .unwrap();
    }

    // Query on the partition key only
    let teams = Team::filter(Team::FIELDS.league().eq("EPL"))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(
        names,
        ["Arsenal", "Chelsea", "Manchester United", "Tottenham"]
    );

    // Query on the partition key and local key
    let teams = Team::filter(
        Team::FIELDS
            .league()
            .eq("MLS")
            .and(Team::FIELDS.name().eq("Portland Timbers")),
    )
    .all(&db)
    .await
    .unwrap()
    .collect::<Vec<_>>()
    .await
    .unwrap();

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers"]);

    // Query on the partition key and a non-index field
    let teams = Team::filter(
        Team::FIELDS
            .league()
            .eq("MLS")
            .and(Team::FIELDS.founded().eq(2009)),
    )
    .all(&db)
    .await
    .unwrap()
    .collect::<Vec<_>>()
    .await
    .unwrap();

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers", "Vancouver Whitecaps FC"]);

    // Query on the partition key, local key, and a non-index field with a match
    let teams = Team::filter(
        Team::FIELDS
            .league()
            .eq("MLS")
            .and(Team::FIELDS.founded().eq(2009))
            .and(Team::FIELDS.name().eq("Portland Timbers")),
    )
    .all(&db)
    .await
    .unwrap()
    .collect::<Vec<_>>()
    .await
    .unwrap();

    assert_eq!(1, teams.len());
    assert!(teams.iter().all(|team| team.founded == 2009));

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers"]);

    // Query on the partition key, local key, and a non-index field without a match
    let teams = Team::filter(
        Team::FIELDS
            .league()
            .eq("MLS")
            .and(Team::FIELDS.founded().eq(2009))
            .and(Team::FIELDS.name().eq("LA Galaxy")),
    )
    .all(&db)
    .await
    .unwrap()
    .collect::<Vec<_>>()
    .await
    .unwrap();

    assert!(teams.is_empty());
}

#[driver_test(id(ID))]
pub async fn query_local_key_cmp(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = timestamp)]
    struct Event {
        kind: String,

        timestamp: i64,
    }

    let db = test.setup_db(models!(Event)).await;

    // Create a bunch of entries
    for (kind, ts) in [
        ("info", 0),
        ("warn", 1),
        ("info", 2),
        ("warn", 3),
        ("info", 4),
        ("warn", 5),
        ("info", 6),
        ("warn", 7),
        ("info", 8),
        ("warn", 9),
        ("info", 10),
        ("warn", 11),
        ("info", 12),
        ("warn", 13),
        ("info", 14),
        ("warn", 15),
        ("info", 16),
        ("warn", 17),
        ("info", 18),
        ("warn", 19),
    ] {
        Event::create()
            .kind(kind)
            .timestamp(ts)
            .exec(&db)
            .await
            .unwrap();
    }

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::FIELDS.timestamp().ne(10))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::FIELDS.timestamp().gt(10))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::FIELDS.timestamp().ge(10))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&10, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::FIELDS.timestamp().lt(10))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::FIELDS.timestamp().le(10))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &10]
    );
}

#[driver_test(id(ID))]
pub async fn query_arbitrary_constraint(test: &mut Test) {
    // Only supported by SQL
    if !test.capability().sql {
        return;
    }

    #[derive(Debug, toasty::Model)]
    struct Event {
        #[key]
        #[auto]
        id: ID,

        kind: String,

        timestamp: i64,
    }

    let db = test.setup_db(models!(Event)).await;

    // Create a bunch of entries
    for (kind, ts) in [
        ("info", 0),
        ("warn", 1),
        ("info", 2),
        ("warn", 3),
        ("info", 4),
        ("warn", 5),
        ("info", 6),
        ("warn", 7),
        ("info", 8),
        ("warn", 9),
        ("info", 10),
        ("warn", 11),
        ("info", 12),
        ("warn", 13),
        ("info", 14),
        ("warn", 15),
        ("info", 16),
        ("warn", 17),
        ("info", 18),
        ("warn", 19),
    ] {
        Event::create()
            .kind(kind)
            .timestamp(ts)
            .exec(&db)
            .await
            .unwrap();
    }

    let events: Vec<_> = Event::filter(Event::FIELDS.timestamp().gt(12))
        .collect(&db)
        .await
        .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&13, &14, &15, &16, &17, &18, &19,]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .timestamp()
            .gt(12)
            .and(Event::FIELDS.kind().ne("info")),
    )
    .collect(&db)
    .await
    .unwrap();

    assert!(events.iter().all(|event| event.kind != "info"));

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&13, &15, &17, &19,]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .kind()
            .eq("info")
            .and(Event::FIELDS.timestamp().ne(10)),
    )
    .collect(&db)
    .await
    .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .kind()
            .eq("info")
            .and(Event::FIELDS.timestamp().gt(10)),
    )
    .collect(&db)
    .await
    .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .kind()
            .eq("info")
            .and(Event::FIELDS.timestamp().ge(10)),
    )
    .collect(&db)
    .await
    .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&10, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .kind()
            .eq("info")
            .and(Event::FIELDS.timestamp().lt(10)),
    )
    .collect(&db)
    .await
    .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8]
    );

    let events: Vec<_> = Event::filter(
        Event::FIELDS
            .kind()
            .eq("info")
            .and(Event::FIELDS.timestamp().le(10)),
    )
    .collect(&db)
    .await
    .unwrap();

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &10]
    );
}
