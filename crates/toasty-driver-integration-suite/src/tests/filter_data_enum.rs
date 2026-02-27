use crate::prelude::*;

/// Filtering by a data-carrying enum value using a SQL WHERE clause.
/// DynamoDB does not support arbitrary filter predicates, so this is SQL-only.
#[driver_test(requires(sql))]
pub async fn filter_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        contact: ContactInfo,
    }

    let db = t.setup_db(models!(User, ContactInfo)).await;

    User::create()
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&db)
        .await?;

    User::create()
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&db)
        .await?;

    let emails = User::filter(User::fields().contact().eq(ContactInfo::Email {
        address: "alice@example.com".to_string(),
    }))
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].name, "Alice");

    Ok(())
}

/// Creates records with different data-carrying enum variants and retrieves them
/// by primary key, verifying enum values round-trip correctly. This exercises
/// the same create + read path on all drivers including DynamoDB.
#[driver_test]
pub async fn create_and_get_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        contact: ContactInfo,
    }

    let db = t.setup_db(models!(User, ContactInfo)).await;

    let alice = User::create()
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&db)
        .await?;

    let bob = User::create()
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&db)
        .await?;

    let found_alice = User::get_by_id(&db, &alice.id).await?;
    assert_eq!(found_alice.name, "Alice");
    assert_eq!(
        found_alice.contact,
        ContactInfo::Email {
            address: "alice@example.com".to_string()
        }
    );

    let found_bob = User::get_by_id(&db, &bob.id).await?;
    assert_eq!(found_bob.name, "Bob");
    assert_eq!(
        found_bob.contact,
        ContactInfo::Phone {
            number: "555-1234".to_string()
        }
    );

    Ok(())
}
