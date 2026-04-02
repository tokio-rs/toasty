use crate::prelude::*;

/// Filtering by a field within a specific enum variant using the closure-based
/// `.matches()` API: `contact().email().matches(|e| e.address().eq("x"))`.
#[driver_test(requires(sql))]
pub async fn filter_by_variant_field(t: &mut Test) -> Result<()> {
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

    let mut db = t.setup_db(toasty::models!(User, ContactInfo)).await;

    User::create()
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .name("Carol")
        .contact(ContactInfo::Email {
            address: "carol@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Filter by email address field
    let results = User::filter(
        User::fields()
            .contact()
            .email()
            .matches(|e| e.address().eq("alice@example.com")),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");

    // Filter by phone number field
    let results = User::filter(
        User::fields()
            .contact()
            .phone()
            .matches(|e| e.number().eq("555-1234")),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Bob");

    // Filter by email address that doesn't match any record
    let results = User::filter(
        User::fields()
            .contact()
            .email()
            .matches(|e| e.address().eq("nobody@example.com")),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(results.len(), 0);

    Ok(())
}

/// Variant+field filter combined with a partition key so DynamoDB can execute it.
#[driver_test]
pub async fn filter_variant_field_with_partition_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = id)]
    #[allow(dead_code)]
    struct User {
        #[auto]
        id: uuid::Uuid,
        group: String,
        name: String,
        contact: ContactInfo,
    }

    let mut db = t.setup_db(toasty::models!(User, ContactInfo)).await;

    User::create()
        .group("eng")
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .group("eng")
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&mut db)
        .await?;

    User::create()
        .group("eng")
        .name("Carol")
        .contact(ContactInfo::Email {
            address: "carol@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Partition key + variant field filter
    let results = User::filter(
        User::fields().group().eq("eng").and(
            User::fields()
                .contact()
                .email()
                .matches(|e| e.address().eq("alice@example.com")),
        ),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");

    Ok(())
}
