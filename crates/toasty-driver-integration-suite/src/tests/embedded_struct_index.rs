use crate::prelude::*;

/// Tests that `#[unique]` and `#[index]` on embedded struct fields produce
/// physical DB indices on the flattened columns.
#[driver_test]
pub async fn embedded_struct_index_schema(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Contact {
        #[unique]
        email: String,
        #[index]
        country: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
        #[allow(dead_code)]
        contact: Contact,
    }

    let db = test.setup_db(models!(User, Contact)).await;
    let schema = db.schema();

    // The embedded struct should carry its indices in the app schema
    assert_struct!(schema.app.models, #{
        Contact::id(): toasty::schema::app::Model::EmbeddedStruct(_ {
            indices.len(): 2,
            ..
        }),
        ..
    });

    // The DB table should have indices on the flattened embedded columns.
    // Index 0: primary key (id)
    // Index 1: unique on contact_email
    // Index 2: non-unique on contact_country
    let table = &schema.db.tables[0];
    let contact_email_col = table
        .columns
        .iter()
        .find(|c| c.name == "contact_email")
        .expect("contact_email column should exist");
    let contact_country_col = table
        .columns
        .iter()
        .find(|c| c.name == "contact_country")
        .expect("contact_country column should exist");

    // Should have 3 indices total: PK + unique email + non-unique country
    assert_eq!(table.indices.len(), 3);

    // Unique index on contact_email
    let email_index = &table.indices[1];
    assert!(email_index.unique);
    assert!(!email_index.primary_key);
    assert_eq!(email_index.columns.len(), 1);
    assert_eq!(email_index.columns[0].column, contact_email_col.id);

    // Non-unique index on contact_country
    let country_index = &table.indices[2];
    assert!(!country_index.unique);
    assert!(!country_index.primary_key);
    assert_eq!(country_index.columns.len(), 1);
    assert_eq!(country_index.columns[0].column, contact_country_col.id);
}

/// Tests that unique constraint on embedded struct field is enforced at the
/// database level and that filtering by indexed embedded fields works.
#[driver_test]
pub async fn embedded_struct_unique_index_enforced(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Contact {
        #[unique]
        email: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
        contact: Contact,
    }

    let mut db = test.setup_db(models!(User, Contact)).await;

    // Create a user with a contact email
    User::create()
        .id("1")
        .name("Alice")
        .contact(Contact {
            email: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Creating another user with the same contact email should fail
    assert_err!(
        User::create()
            .id("2")
            .name("Bob")
            .contact(Contact {
                email: "alice@example.com".to_string(),
            })
            .exec(&mut db)
            .await
    );

    // Creating a user with a different email works
    User::create()
        .id("3")
        .name("Charlie")
        .contact(Contact {
            email: "charlie@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Filter by the indexed embedded field
    let users = User::filter(User::fields().contact().email().eq("alice@example.com"))
        .all(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// Tests that `#[index]` on a field inside a nested embedded struct (embed
/// within embed) produces a physical DB index on the deeply-flattened column.
#[driver_test]
pub async fn nested_embedded_struct_index(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Geo {
        #[index]
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        #[allow(dead_code)]
        geo: Geo,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        address: Address,
    }

    let db = test.setup_db(models!(User, Address, Geo)).await;
    let schema = db.schema();

    let table = &schema.db.tables[0];

    // The nested embedded field should be flattened to address_geo_city
    let city_col = table
        .columns
        .iter()
        .find(|c| c.name == "address_geo_city")
        .expect("address_geo_city column should exist");

    // Should have 2 indices: PK + non-unique on address_geo_city
    assert_eq!(table.indices.len(), 2);

    let city_index = &table.indices[1];
    assert!(!city_index.unique);
    assert_eq!(city_index.columns.len(), 1);
    assert_eq!(city_index.columns[0].column, city_col.id);
}
