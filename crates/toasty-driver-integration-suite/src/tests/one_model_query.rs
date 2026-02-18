//! Test querying models with various filters and constraints

use crate::prelude::*;
use toasty_core::{
    driver::Operation,
    stmt::{Expr, ExprSet, Statement},
};

#[driver_test(id(ID))]
pub async fn query_index_eq(test: &mut Test) -> Result<()> {
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
        User::create().name(name).email(email).exec(&db).await?;
    }

    let users = User::filter_by_name("one").collect::<Vec<_>>(&db).await?;

    assert_eq!(1, users.len());
    assert_eq!("one", users[0].name);

    // Create a second user named "one"

    User::create()
        .name("one")
        .email("one-two@example.com")
        .exec(&db)
        .await?;

    let mut users = User::filter_by_name("one")
        .all(&db)
        .await?
        .collect::<Vec<_>>()
        .await?;

    users.sort_by_key(|u| u.email.clone());

    assert_eq!(2, users.len());
    assert_eq!("one", users[0].name);
    assert_eq!("one-two@example.com", users[0].email);

    assert_eq!("one", users[1].name);
    assert_eq!("one@example.com", users[1].email);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_partition_key_string_eq(test: &mut Test) -> Result<()> {
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
            .await?;
    }

    // Query on the partition key only
    let teams = Team::filter(Team::fields().league().eq("EPL"))
        .collect::<Vec<_>>(&db)
        .await?;

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(
        names,
        ["Arsenal", "Chelsea", "Manchester United", "Tottenham"]
    );

    // Query on the partition key and local key
    let teams = Team::filter(
        Team::fields()
            .league()
            .eq("MLS")
            .and(Team::fields().name().eq("Portland Timbers")),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers"]);

    // Query on the partition key and a non-index field
    let teams = Team::filter(
        Team::fields()
            .league()
            .eq("MLS")
            .and(Team::fields().founded().eq(2009)),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers", "Vancouver Whitecaps FC"]);

    // Query on the partition key, local key, and a non-index field with a match
    let teams = Team::filter(
        Team::fields()
            .league()
            .eq("MLS")
            .and(Team::fields().founded().eq(2009))
            .and(Team::fields().name().eq("Portland Timbers")),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(1, teams.len());
    assert!(teams.iter().all(|team| team.founded == 2009));

    let mut names = teams.iter().map(|team| &team.name).collect::<Vec<_>>();
    names.sort();

    assert_eq!(names, ["Portland Timbers"]);

    // Query on the partition key, local key, and a non-index field without a match
    let teams = Team::filter(
        Team::fields()
            .league()
            .eq("MLS")
            .and(Team::fields().founded().eq(2009))
            .and(Team::fields().name().eq("LA Galaxy")),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert!(teams.is_empty());
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_local_key_cmp(test: &mut Test) -> Result<()> {
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
        Event::create().kind(kind).timestamp(ts).exec(&db).await?;
    }

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::fields().timestamp().ne(10))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::fields().timestamp().gt(10))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::fields().timestamp().ge(10))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&10, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::fields().timestamp().lt(10))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8]
    );

    let events: Vec<_> = Event::filter_by_kind("info")
        .filter(Event::fields().timestamp().le(10))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &10]
    );
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_or_basic(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,
    }

    let db = test.setup_db(models!(User)).await;
    let _name_column = db.schema().table_for(User::id()).columns[1].id;
    let _age_column = db.schema().table_for(User::id()).columns[2].id;

    // Create some users
    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 35), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&db).await?;
    }

    // Clear the log after setup
    test.log().clear();

    // Query with OR condition: name = "Alice" OR age = 35
    let result = User::filter(
        User::fields()
            .name()
            .eq("Alice")
            .or(User::fields().age().eq(35)),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(2, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Alice", "Charlie"]);

        // Verify the driver operation contains the expected OR filter
        let (op, _) = test.log().pop();

        assert_struct!(&op, Operation::QuerySql(_ {
            stmt: Statement::Query(_ {
                body: ExprSet::Select(_ {
                    filter.expr: Some(Expr::Or(_ {
                        // TODO: assert_struct! needs a set matcher
                        /*
                        operands: [
                            Expr::BinaryOp(_ {
                                op: BinaryOp::Eq,
                                *lhs: == Expr::column(age_column),
                                *rhs: Expr::Value(Value::I64(35)),
                                ..
                            }),
                            Expr::BinaryOp(_ {
                                op: BinaryOp::Eq,
                                *lhs: == Expr::column(name_column),
                                *rhs: Expr::Value(Value::String("Alice")),
                                ..
                            }),
                        ],
                        */
                        ..
                    })),
                    ..
                }),
                ..
            }),
            ..
        }));
    } else {
        // DynamoDB requires key conditions for queries - OR filters without
        // key conditions should return an error
        assert!(
            result.is_err(),
            "Expected error for OR query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_or_multiple(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,
    }

    let db = test.setup_db(models!(User)).await;

    // Create some users
    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 35), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&db).await?;
    }

    // Query with multiple OR conditions: name = "Alice" OR age = 35 OR age = 40
    let result = User::filter(
        User::fields()
            .name()
            .eq("Alice")
            .or(User::fields().age().eq(35))
            .or(User::fields().age().eq(40)),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(3, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Alice", "Charlie", "Diana"]);
    } else {
        // DynamoDB requires key conditions for queries - OR filters without
        // key conditions should return an error
        assert!(
            result.is_err(),
            "Expected error for OR query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_or_and_combined(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,

        #[allow(dead_code)]
        active: bool,
    }

    let db = test.setup_db(models!(User)).await;

    // Create some users
    for (name, age, active) in [
        ("Alice", 25, true),
        ("Bob", 30, false),
        ("Charlie", 35, true),
        ("Diana", 40, false),
        ("Eve", 25, false),
    ] {
        User::create()
            .name(name)
            .age(age)
            .active(active)
            .exec(&db)
            .await?;
    }

    // Query with OR and AND: (name = "Alice" OR age = 35) AND active = true
    let result = User::filter(
        User::fields()
            .name()
            .eq("Alice")
            .or(User::fields().age().eq(35))
            .and(User::fields().active().eq(true)),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(2, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Alice", "Charlie"]);
    } else {
        // DynamoDB requires key conditions for queries - OR filters without
        // key conditions should return an error
        assert!(
            result.is_err(),
            "Expected error for OR query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_or_with_index(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        #[allow(dead_code)]
        position: String,

        #[allow(dead_code)]
        number: i64,
    }

    let db = test.setup_db(models!(Player)).await;

    // Create some players on different teams
    for (team, name, position, number) in [
        ("Timbers", "Diego Valeri", "Midfielder", 8),
        ("Timbers", "Darlington Nagbe", "Midfielder", 6),
        ("Timbers", "Diego Chara", "Midfielder", 21),
        ("Timbers", "Fanendo Adi", "Forward", 9),
        ("Timbers", "Adam Kwarasey", "Goalkeeper", 1),
        ("Sounders", "Clint Dempsey", "Forward", 2),
        ("Sounders", "Obafemi Martins", "Forward", 9),
        ("Sounders", "Osvaldo Alonso", "Midfielder", 6),
    ] {
        Player::create()
            .team(team)
            .name(name)
            .position(position)
            .number(number)
            .exec(&db)
            .await?;
    }

    // Query with partition key AND OR conditions on non-indexed fields
    // This should work on both SQL and DynamoDB
    let players = Player::filter(
        Player::fields().team().eq("Timbers").and(
            Player::fields()
                .position()
                .eq("Forward")
                .or(Player::fields().position().eq("Goalkeeper")),
        ),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(2, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Fanendo Adi"]);

    // Query with partition key AND more complex OR conditions
    let players = Player::filter(
        Player::fields().team().eq("Timbers").and(
            Player::fields()
                .number()
                .eq(8)
                .or(Player::fields().number().eq(21))
                .or(Player::fields().number().eq(9)),
        ),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(3, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Diego Chara", "Diego Valeri", "Fanendo Adi"]);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_or_with_comparisons(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        #[allow(dead_code)]
        position: String,

        #[allow(dead_code)]
        number: i64,
    }

    let db = test.setup_db(models!(Player)).await;

    // Create some players on different teams
    for (team, name, position, number) in [
        ("Timbers", "Diego Valeri", "Midfielder", 8),
        ("Timbers", "Darlington Nagbe", "Midfielder", 6),
        ("Timbers", "Diego Chara", "Midfielder", 21),
        ("Timbers", "Fanendo Adi", "Forward", 9),
        ("Timbers", "Adam Kwarasey", "Goalkeeper", 1),
        ("Sounders", "Clint Dempsey", "Forward", 2),
        ("Sounders", "Obafemi Martins", "Forward", 9),
        ("Sounders", "Osvaldo Alonso", "Midfielder", 6),
    ] {
        Player::create()
            .team(team)
            .name(name)
            .position(position)
            .number(number)
            .exec(&db)
            .await?;
    }

    // Query with partition key AND OR conditions using comparisons (not equality)
    // This won't be optimized to IN list, so tests actual OR expression handling
    // Using gt/lt instead of ge/le to avoid boundary condition confusion
    let players = Player::filter(
        Player::fields().team().eq("Timbers").and(
            Player::fields()
                .number()
                .gt(20)
                .or(Player::fields().number().lt(2)),
        ),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(2, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Diego Chara"]);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_arbitrary_constraint(test: &mut Test) -> Result<()> {
    // Only supported by SQL
    if !test.capability().sql {
        return Ok(());
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
        Event::create().kind(kind).timestamp(ts).exec(&db).await?;
    }

    let events: Vec<_> = Event::filter(Event::fields().timestamp().gt(12))
        .collect(&db)
        .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&13, &14, &15, &16, &17, &18, &19,]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .timestamp()
            .gt(12)
            .and(Event::fields().kind().ne("info")),
    )
    .collect(&db)
    .await?;

    assert!(events.iter().all(|event| event.kind != "info"));

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&13, &15, &17, &19,]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .kind()
            .eq("info")
            .and(Event::fields().timestamp().ne(10)),
    )
    .collect(&db)
    .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .kind()
            .eq("info")
            .and(Event::fields().timestamp().gt(10)),
    )
    .collect(&db)
    .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .kind()
            .eq("info")
            .and(Event::fields().timestamp().ge(10)),
    )
    .collect(&db)
    .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&10, &12, &14, &16, &18,]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .kind()
            .eq("info")
            .and(Event::fields().timestamp().lt(10)),
    )
    .collect(&db)
    .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8]
    );

    let events: Vec<_> = Event::filter(
        Event::fields()
            .kind()
            .eq("info")
            .and(Event::fields().timestamp().le(10)),
    )
    .collect(&db)
    .await?;

    assert_eq_unordered!(
        events.iter().map(|event| event.timestamp),
        [&0, &2, &4, &6, &8, &10]
    );
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_not_basic(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,
    }

    let db = test.setup_db(models!(User)).await;

    // Create some users
    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 35), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&db).await?;
    }

    // Clear the log after setup
    test.log().clear();

    // Query with NOT condition: NOT (name = "Alice")
    let result = User::filter(User::fields().name().eq("Alice").not())
        .collect::<Vec<_>>(&db)
        .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(3, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Bob", "Charlie", "Diana"]);
    } else {
        // DynamoDB requires key conditions for queries - NOT filters without
        // key conditions should return an error
        assert!(
            result.is_err(),
            "Expected error for NOT query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_not_and_combined(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,

        #[allow(dead_code)]
        active: bool,
    }

    let db = test.setup_db(models!(User)).await;

    // Create some users
    for (name, age, active) in [
        ("Alice", 25, true),
        ("Bob", 30, false),
        ("Charlie", 35, true),
        ("Diana", 40, false),
        ("Eve", 25, false),
    ] {
        User::create()
            .name(name)
            .age(age)
            .active(active)
            .exec(&db)
            .await?;
    }

    // Query with NOT combined with AND: active = true AND NOT (age = 25)
    // Should return only Charlie (active=true, age=35)
    let result = User::filter(
        User::fields()
            .active()
            .eq(true)
            .and(User::fields().age().eq(25).not()),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(1, users.len());
        assert_eq!("Charlie", users[0].name);
    } else {
        assert!(
            result.is_err(),
            "Expected error for NOT query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_not_or_combined(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[allow(dead_code)]
        age: i64,
    }

    let db = test.setup_db(models!(User)).await;

    // Create some users
    for (name, age) in [("Alice", 25), ("Bob", 30), ("Charlie", 35), ("Diana", 40)] {
        User::create().name(name).age(age).exec(&db).await?;
    }

    // Query with NOT combined with OR: NOT (name = "Alice" OR name = "Bob")
    // Should return Charlie and Diana
    let result = User::filter(
        User::fields()
            .name()
            .eq("Alice")
            .or(User::fields().name().eq("Bob"))
            .not(),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(2, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Charlie", "Diana"]);
    } else {
        assert!(
            result.is_err(),
            "Expected error for NOT query without key condition on non-SQL database"
        );
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_not_with_index(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        #[allow(dead_code)]
        position: String,

        #[allow(dead_code)]
        number: i64,
    }

    let db = test.setup_db(models!(Player)).await;

    // Create some players
    for (team, name, position, number) in [
        ("Timbers", "Diego Valeri", "Midfielder", 8),
        ("Timbers", "Darlington Nagbe", "Midfielder", 6),
        ("Timbers", "Diego Chara", "Midfielder", 21),
        ("Timbers", "Fanendo Adi", "Forward", 9),
        ("Timbers", "Adam Kwarasey", "Goalkeeper", 1),
        ("Sounders", "Clint Dempsey", "Forward", 2),
        ("Sounders", "Obafemi Martins", "Forward", 9),
        ("Sounders", "Osvaldo Alonso", "Midfielder", 6),
    ] {
        Player::create()
            .team(team)
            .name(name)
            .position(position)
            .number(number)
            .exec(&db)
            .await?;
    }

    // Query with partition key AND NOT condition on non-indexed field
    // team = "Timbers" AND NOT (position = "Midfielder")
    // Should return Fanendo Adi (Forward) and Adam Kwarasey (Goalkeeper)
    let players = Player::filter(
        Player::fields()
            .team()
            .eq("Timbers")
            .and(Player::fields().position().eq("Midfielder").not()),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(2, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Fanendo Adi"]);

    // Query with partition key AND NOT with comparison
    // team = "Timbers" AND NOT (number > 8)
    // Should return players with number <= 8: Diego Valeri (8), Darlington Nagbe (6), Adam Kwarasey (1)
    let players = Player::filter(
        Player::fields()
            .team()
            .eq("Timbers")
            .and(Player::fields().number().gt(8).not()),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(3, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Darlington Nagbe", "Diego Valeri"]);
    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_not_operator_syntax(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        #[allow(dead_code)]
        position: String,

        #[allow(dead_code)]
        number: i64,
    }

    let db = test.setup_db(models!(Player)).await;

    for (team, name, position, number) in [
        ("Timbers", "Diego Valeri", "Midfielder", 8),
        ("Timbers", "Darlington Nagbe", "Midfielder", 6),
        ("Timbers", "Diego Chara", "Midfielder", 21),
        ("Timbers", "Fanendo Adi", "Forward", 9),
        ("Timbers", "Adam Kwarasey", "Goalkeeper", 1),
    ] {
        Player::create()
            .team(team)
            .name(name)
            .position(position)
            .number(number)
            .exec(&db)
            .await?;
    }

    // Use the ! operator instead of .not()
    // team = "Timbers" AND !(position = "Midfielder")
    let players = Player::filter(
        Player::fields()
            .team()
            .eq("Timbers")
            .and(!Player::fields().position().eq("Midfielder")),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(2, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Fanendo Adi"]);

    // ! on a compound expression: !(number > 8 OR position = "Goalkeeper")
    let players = Player::filter(
        Player::fields().team().eq("Timbers").and(
            !(Player::fields()
                .number()
                .gt(8)
                .or(Player::fields().position().eq("Goalkeeper"))),
        ),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    // Excludes Diego Chara (21), Fanendo Adi (9), Adam Kwarasey (Goalkeeper)
    // Keeps Diego Valeri (8, Midfielder), Darlington Nagbe (6, Midfielder)
    assert_eq!(2, players.len());
    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Darlington Nagbe", "Diego Valeri"]);
    Ok(())
}
