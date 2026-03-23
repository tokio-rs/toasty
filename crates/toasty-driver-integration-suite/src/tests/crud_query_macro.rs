use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_all(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    // query!(User) expands to User::all()
    let users = toasty::query!(User).exec(&mut db).await?;
    assert_eq!(users.len(), 2);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_eq(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    // query!(User filter .name == "Alice") expands to User::filter(User::fields().name().eq("Alice"))
    let users = toasty::query!(User filter .name == "Alice")
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_ne(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let users = toasty::query!(User filter .name != "Alice")
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn query_macro_filter_numeric_comparisons(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    toasty::create!(User {
        name: "Young",
        age: 15
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Adult",
        age: 25
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Senior",
        age: 65
    })
    .exec(&mut db)
    .await?;

    // Greater than
    let users = toasty::query!(User filter .age > 20).exec(&mut db).await?;
    assert_eq!(users.len(), 2);

    // Greater than or equal
    let users = toasty::query!(User filter .age >= 25).exec(&mut db).await?;
    assert_eq!(users.len(), 2);

    // Less than
    let users = toasty::query!(User filter .age < 25).exec(&mut db).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Young");

    // Less than or equal
    let users = toasty::query!(User filter .age <= 25).exec(&mut db).await?;
    assert_eq!(users.len(), 2);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn query_macro_filter_and(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        #[index]
        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    toasty::create!(User {
        name: "Alice",
        age: 30
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        age: 30
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Alice",
        age: 20
    })
    .exec(&mut db)
    .await?;

    let users = toasty::query!(User filter .name == "Alice" and .age == 30)
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    assert_eq!(users[0].age, 30);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_or(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;
    toasty::create!(User { name: "Carl" }).exec(&mut db).await?;

    let users = toasty::query!(User filter .name == "Alice" or .name == "Bob")
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 2);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_not(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let users = toasty::query!(User filter not .name == "Alice")
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn query_macro_filter_parens(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        #[index]
        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    toasty::create!(User {
        name: "Alice",
        age: 30
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        age: 20
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Carl",
        age: 40
    })
    .exec(&mut db)
    .await?;

    // AND binds tighter than OR, so parentheses change the grouping:
    // .name == "Alice" AND (.age > 25 OR .age < 15)
    let users = toasty::query!(User filter .name == "Alice" and (.age > 25 or .age < 15))
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_external_ref(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    let target_name = "Alice";
    let users = toasty::query!(User filter .name == #target_name)
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_filter_external_expr(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    fn get_name() -> &'static str {
        "Bob"
    }

    let users = toasty::query!(User filter .name == #(get_name()))
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::two_models), requires(sql))]
pub async fn query_macro_case_insensitive_keywords(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(User { name: "Bob" }).exec(&mut db).await?;

    // FILTER (uppercase)
    let users = toasty::query!(User FILTER .name == "Alice")
        .exec(&mut db)
        .await?;
    assert_eq!(users.len(), 1);

    // Filter (mixed case), AND, OR
    let users = toasty::query!(User Filter .name == "Alice" AND .name == "Alice")
        .exec(&mut db)
        .await?;
    assert_eq!(users.len(), 1);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn query_macro_complex_boolean(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        #[index]
        name: String,

        #[index]
        age: i64,
    }

    let mut db = test.setup_db(models!(User)).await;

    toasty::create!(User {
        name: "Alice",
        age: 30
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        age: 20
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Carl",
        age: 40
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Diana",
        age: 10
    })
    .exec(&mut db)
    .await?;

    // Complex: NOT (.age < 18) AND (.name == "Alice" OR .name == "Carl")
    let users =
        toasty::query!(User filter not (.age < 18) and (.name == "Alice" or .name == "Carl"))
            .exec(&mut db)
            .await?;

    assert_eq!(users.len(), 2);
    let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["Alice", "Carl"]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
pub async fn query_macro_filter_bool_literal(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        name: String,

        #[index]
        active: bool,
    }

    let mut db = test.setup_db(models!(Item)).await;

    toasty::create!(Item {
        name: "on",
        active: true
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Item {
        name: "off",
        active: false
    })
    .exec(&mut db)
    .await?;

    let items = toasty::query!(Item filter .active == true)
        .exec(&mut db)
        .await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "on");

    Ok(())
}

// --- DynamoDB-compatible query macro tests ---
// These use composite primary keys (partition + local) so queries can be served
// by DynamoDB's key condition expressions.

#[driver_test(id(ID))]
pub async fn query_macro_partition_key_eq(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = league, local = name)]
    struct Team {
        league: String,

        name: String,

        founded: i64,
    }

    let mut db = test.setup_db(models!(Team)).await;

    for (league, name, founded) in [
        ("MLS", "Portland Timbers", 2009),
        ("MLS", "Seattle Sounders FC", 2007),
        ("EPL", "Arsenal", 1886),
        ("EPL", "Chelsea", 1905),
    ] {
        toasty::create!(Team {
            league: league,
            name: name,
            founded: founded
        })
        .exec(&mut db)
        .await?;
    }

    // Filter on partition key only
    let teams = toasty::query!(Team filter .league == "EPL")
        .exec(&mut db)
        .await?;

    let mut names: Vec<_> = teams.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Arsenal", "Chelsea"]);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_macro_partition_and_local_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = league, local = name)]
    struct Team {
        league: String,

        name: String,

        founded: i64,
    }

    let mut db = test.setup_db(models!(Team)).await;

    for (league, name, founded) in [
        ("MLS", "Portland Timbers", 2009),
        ("MLS", "Seattle Sounders FC", 2007),
        ("EPL", "Arsenal", 1886),
        ("EPL", "Chelsea", 1905),
    ] {
        toasty::create!(Team {
            league: league,
            name: name,
            founded: founded
        })
        .exec(&mut db)
        .await?;
    }

    // Filter on partition key + local key
    let teams = toasty::query!(Team filter .league == "MLS" and .name == "Portland Timbers")
        .exec(&mut db)
        .await?;

    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0].name, "Portland Timbers");
    assert_eq!(teams[0].founded, 2009);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_macro_local_key_comparison(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = kind, local = timestamp)]
    struct Event {
        kind: String,

        timestamp: i64,
    }

    let mut db = test.setup_db(models!(Event)).await;

    for (kind, ts) in [
        ("info", 0),
        ("info", 2),
        ("info", 4),
        ("info", 6),
        ("info", 8),
        ("info", 10),
        ("warn", 1),
        ("warn", 3),
        ("warn", 5),
    ] {
        toasty::create!(Event {
            kind: kind,
            timestamp: ts
        })
        .exec(&mut db)
        .await?;
    }

    // Partition key + greater-than on local key
    let events = toasty::query!(Event filter .kind == "info" and .timestamp > 6)
        .exec(&mut db)
        .await?;

    let mut timestamps: Vec<_> = events.iter().map(|e| e.timestamp).collect();
    timestamps.sort();
    assert_eq!(timestamps, [8, 10]);

    // Partition key + less-than-or-equal on local key
    let events = toasty::query!(Event filter .kind == "info" and .timestamp <= 4)
        .exec(&mut db)
        .await?;

    let mut timestamps: Vec<_> = events.iter().map(|e| e.timestamp).collect();
    timestamps.sort();
    assert_eq!(timestamps, [0, 2, 4]);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_macro_partition_key_external_ref(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = league, local = name)]
    struct Team {
        league: String,

        name: String,

        founded: i64,
    }

    let mut db = test.setup_db(models!(Team)).await;

    for (league, name, founded) in [
        ("MLS", "Portland Timbers", 2009),
        ("MLS", "Seattle Sounders FC", 2007),
        ("EPL", "Arsenal", 1886),
    ] {
        toasty::create!(Team {
            league: league,
            name: name,
            founded: founded
        })
        .exec(&mut db)
        .await?;
    }

    // Use external variable reference with partition key query
    let target_league = "MLS";
    let teams = toasty::query!(Team filter .league == #target_league)
        .exec(&mut db)
        .await?;

    let mut names: Vec<_> = teams.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Portland Timbers", "Seattle Sounders FC"]);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_macro_partition_key_with_not(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        position: String,
    }

    let mut db = test.setup_db(models!(Player)).await;

    for (team, name, position) in [
        ("Timbers", "Diego Valeri", "Midfielder"),
        ("Timbers", "Fanendo Adi", "Forward"),
        ("Timbers", "Adam Kwarasey", "Goalkeeper"),
        ("Sounders", "Clint Dempsey", "Forward"),
    ] {
        toasty::create!(Player {
            team: team,
            name: name,
            position: position
        })
        .exec(&mut db)
        .await?;
    }

    // Partition key + NOT on non-key field
    let players =
        toasty::query!(Player filter .team == "Timbers" and not .position == "Midfielder")
            .exec(&mut db)
            .await?;

    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Fanendo Adi"]);

    Ok(())
}

#[driver_test(id(ID))]
pub async fn query_macro_partition_key_with_or(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = team, local = name)]
    struct Player {
        team: String,

        name: String,

        position: String,

        number: i64,
    }

    let mut db = test.setup_db(models!(Player)).await;

    for (team, name, position, number) in [
        ("Timbers", "Diego Valeri", "Midfielder", 8),
        ("Timbers", "Darlington Nagbe", "Midfielder", 6),
        ("Timbers", "Fanendo Adi", "Forward", 9),
        ("Timbers", "Adam Kwarasey", "Goalkeeper", 1),
        ("Sounders", "Clint Dempsey", "Forward", 2),
    ] {
        toasty::create!(Player {
            team: team,
            name: name,
            position: position,
            number: number
        })
        .exec(&mut db)
        .await?;
    }

    // Partition key + OR on non-key fields
    let players = toasty::query!(Player filter .team == "Timbers" and (.position == "Forward" or .position == "Goalkeeper"))
        .exec(&mut db)
        .await?;

    let mut names: Vec<_> = players.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Adam Kwarasey", "Fanendo Adi"]);

    Ok(())
}
