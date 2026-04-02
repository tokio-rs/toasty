use crate::prelude::*;

/// Tests that `#[unique]` and `#[index]` on embedded enum variant fields produce
/// physical DB indices on the flattened columns.
#[driver_test]
pub async fn embedded_enum_index_schema(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email {
            #[unique]
            address: String,
        },
        #[column(variant = 2)]
        Phone {
            #[index]
            number: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
        #[allow(dead_code)]
        contact: ContactInfo,
    }

    let db = test.setup_db(models!(User, ContactInfo)).await;
    let schema = db.schema();

    // The embedded enum should carry its indices in the app schema
    assert_struct!(schema.app.models, #{
        ContactInfo::id(): toasty::schema::app::Model::EmbeddedEnum({
            indices.len(): 2,
        }),
        ..
    });

    // The DB table should have indices on the flattened variant field columns.
    // Index 0: primary key (id)
    // Index 1: unique on contact_address
    // Index 2: non-unique on contact_number
    let table = &schema.db.tables[0];
    let address_col = columns(&db, "users", &["contact_address"])[0];
    let number_col = columns(&db, "users", &["contact_number"])[0];

    assert_struct!(table.indices, [
        // PK
        { primary_key: true },
        // Unique index on contact_address
        { unique: true, primary_key: false, columns: [{ column: == address_col }] },
        // Non-unique index on contact_number
        { unique: false, primary_key: false, columns: [{ column: == number_col }] },
    ]);
}

/// Tests that unique constraint on embedded enum variant field is enforced at
/// the database level.
#[driver_test]
pub async fn embedded_enum_unique_index_enforced(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email {
            #[unique]
            address: String,
        },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
        contact: ContactInfo,
    }

    let mut db = test.setup_db(models!(User, ContactInfo)).await;

    // Create a user with an email contact
    User::create()
        .id("1")
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Creating another user with the same email address should fail
    assert_err!(
        User::create()
            .id("2")
            .name("Bob")
            .contact(ContactInfo::Email {
                address: "alice@example.com".to_string(),
            })
            .exec(&mut db)
            .await
    );

    // Creating a user with a different email works
    User::create()
        .id("3")
        .name("Charlie")
        .contact(ContactInfo::Email {
            address: "charlie@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Creating a user with a phone contact works (different variant, no unique on number)
    User::create()
        .id("4")
        .name("Dave")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Filter by the indexed variant field
    let users = User::filter(
        User::fields()
            .contact()
            .email()
            .matches(|e| e.address().eq("alice@example.com")),
    )
    .exec(&mut db)
    .await?;

    assert_struct!(users, [{ name: "Alice" }]);

    Ok(())
}
