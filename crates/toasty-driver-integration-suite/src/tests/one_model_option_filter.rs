//! Test filtering models by Option fields using is_some() and is_none()

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn filter_option_is_none(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        bio: Option<String>,
    }

    let db = test.setup_db(models!(User)).await;

    // Create users with and without bio
    User::create()
        .name("Alice")
        .bio("Likes Rust")
        .exec(&db)
        .await?;

    User::create().name("Bob").exec(&db).await?;

    User::create()
        .name("Charlie")
        .bio("Likes databases")
        .exec(&db)
        .await?;

    // Filter for users with no bio (IS NULL)
    let result = User::filter(User::fields().bio().is_none())
        .collect::<Vec<_>>(&db)
        .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(1, users.len());
        assert_eq!("Bob", users[0].name);
    } else {
        // DynamoDB doesn't support arbitrary filters without key conditions
        assert!(result.is_err());
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn filter_option_is_some(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        bio: Option<String>,
    }

    let db = test.setup_db(models!(User)).await;

    // Create users with and without bio
    User::create()
        .name("Alice")
        .bio("Likes Rust")
        .exec(&db)
        .await?;

    User::create().name("Bob").exec(&db).await?;

    User::create()
        .name("Charlie")
        .bio("Likes databases")
        .exec(&db)
        .await?;

    // Filter for users with a bio (IS NOT NULL)
    let result = User::filter(User::fields().bio().is_some())
        .collect::<Vec<_>>(&db)
        .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(2, users.len());
        let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["Alice", "Charlie"]);
    } else {
        assert!(result.is_err());
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn filter_option_combined_with_other_filters(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,

        bio: Option<String>,

        #[allow(dead_code)]
        age: i64,
    }

    let db = test.setup_db(models!(User)).await;

    User::create()
        .name("Alice")
        .bio("Likes Rust")
        .age(25)
        .exec(&db)
        .await?;

    User::create().name("Bob").age(30).exec(&db).await?;

    User::create()
        .name("Charlie")
        .bio("Likes databases")
        .age(35)
        .exec(&db)
        .await?;

    User::create().name("Diana").age(25).exec(&db).await?;

    // Combine is_some with an equality filter: has bio AND age > 30
    let result = User::filter(
        User::fields()
            .bio()
            .is_some()
            .and(User::fields().age().gt(30)),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(1, users.len());
        assert_eq!("Charlie", users[0].name);
    } else {
        assert!(result.is_err());
    }

    // Combine is_none with an equality filter: no bio AND age = 25
    let result = User::filter(
        User::fields()
            .bio()
            .is_none()
            .and(User::fields().age().eq(25)),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let users = result?;
        assert_eq!(1, users.len());
        assert_eq!("Diana", users[0].name);
    } else {
        assert!(result.is_err());
    }
    Ok(())
}

#[driver_test(id(ID))]
pub async fn filter_option_multiple_nullable_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: ID,

        title: String,

        subtitle: Option<String>,

        summary: Option<String>,
    }

    let db = test.setup_db(models!(Article)).await;

    // Both set
    Article::create()
        .title("A")
        .subtitle("sub-A")
        .summary("sum-A")
        .exec(&db)
        .await?;

    // Only subtitle set
    Article::create()
        .title("B")
        .subtitle("sub-B")
        .exec(&db)
        .await?;

    // Only summary set
    Article::create()
        .title("C")
        .summary("sum-C")
        .exec(&db)
        .await?;

    // Neither set
    Article::create().title("D").exec(&db).await?;

    // Filter: subtitle is_some AND summary is_none
    let result = Article::filter(
        Article::fields()
            .subtitle()
            .is_some()
            .and(Article::fields().summary().is_none()),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let articles = result?;
        assert_eq!(1, articles.len());
        assert_eq!("B", articles[0].title);
    } else {
        assert!(result.is_err());
    }

    // Filter: both are none
    let result = Article::filter(
        Article::fields()
            .subtitle()
            .is_none()
            .and(Article::fields().summary().is_none()),
    )
    .collect::<Vec<_>>(&db)
    .await;

    if test.capability().sql {
        let articles = result?;
        assert_eq!(1, articles.len());
        assert_eq!("D", articles[0].title);
    } else {
        assert!(result.is_err());
    }

    Ok(())
}
