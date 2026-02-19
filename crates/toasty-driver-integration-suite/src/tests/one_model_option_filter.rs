//! Test filtering models by Option fields using is_some() and is_none()

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
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
    let users = User::filter(User::fields().bio().is_none())
        .collect::<Vec<_>>(&db)
        .await?;

    assert_eq!(1, users.len());
    assert_eq!("Bob", users[0].name);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
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
    let users = User::filter(User::fields().bio().is_some())
        .collect::<Vec<_>>(&db)
        .await?;

    assert_eq!(2, users.len());
    let mut names: Vec<_> = users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Alice", "Charlie"]);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
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
    let users = User::filter(
        User::fields()
            .bio()
            .is_some()
            .and(User::fields().age().gt(30)),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(1, users.len());
    assert_eq!("Charlie", users[0].name);

    // Combine is_none with an equality filter: no bio AND age = 25
    let users = User::filter(
        User::fields()
            .bio()
            .is_none()
            .and(User::fields().age().eq(25)),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(1, users.len());
    assert_eq!("Diana", users[0].name);

    Ok(())
}

#[driver_test(id(ID), requires(sql))]
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
    let articles = Article::filter(
        Article::fields()
            .subtitle()
            .is_some()
            .and(Article::fields().summary().is_none()),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(1, articles.len());
    assert_eq!("B", articles[0].title);

    // Filter: both are none
    let articles = Article::filter(
        Article::fields()
            .subtitle()
            .is_none()
            .and(Article::fields().summary().is_none()),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(1, articles.len());
    assert_eq!("D", articles[0].title);

    Ok(())
}

#[driver_test]
pub async fn filter_option_with_partition_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = category, local = name)]
    struct Product {
        category: String,

        name: String,

        description: Option<String>,
    }

    let db = test.setup_db(models!(Product)).await;

    // Create products in the "Electronics" category
    Product::create()
        .category("Electronics")
        .name("Laptop")
        .description("A powerful laptop")
        .exec(&db)
        .await?;

    Product::create()
        .category("Electronics")
        .name("Mouse")
        .exec(&db)
        .await?;

    Product::create()
        .category("Electronics")
        .name("Keyboard")
        .description("Mechanical keyboard")
        .exec(&db)
        .await?;

    // Create products in the "Books" category
    Product::create()
        .category("Books")
        .name("Rust Programming")
        .description("Learn Rust")
        .exec(&db)
        .await?;

    Product::create()
        .category("Books")
        .name("Cooking 101")
        .exec(&db)
        .await?;

    // Filter by partition key AND description is_none
    let products = Product::filter(
        Product::fields()
            .category()
            .eq("Electronics")
            .and(Product::fields().description().is_none()),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(1, products.len());
    assert_eq!("Mouse", products[0].name);

    // Filter by partition key AND description is_some
    let products = Product::filter(
        Product::fields()
            .category()
            .eq("Electronics")
            .and(Product::fields().description().is_some()),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(2, products.len());
    let mut names: Vec<_> = products.iter().map(|p| p.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Keyboard", "Laptop"]);

    // Filter by a different partition key AND description is_none
    let products = Product::filter(
        Product::fields()
            .category()
            .eq("Books")
            .and(Product::fields().description().is_none()),
    )
    .all(&db)
    .await?
    .collect::<Vec<_>>()
    .await?;

    assert_eq!(1, products.len());
    assert_eq!("Cooking 101", products[0].name);

    Ok(())
}
