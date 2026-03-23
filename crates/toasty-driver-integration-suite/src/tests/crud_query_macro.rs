use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID))]
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

#[driver_test(id(ID))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
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

#[driver_test(id(ID))]
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

#[driver_test(id(ID))]
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
