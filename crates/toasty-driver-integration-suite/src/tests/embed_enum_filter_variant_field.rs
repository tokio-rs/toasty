use crate::prelude::*;

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

    let mut db = t.setup_db(models!(User)).await;

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

/// OR of two variant-rooted field predicates — one per variant. Each predicate
/// on its own works; only the `OR` used to panic in the SQL serializer with
/// "unexpected enum discriminant" (issue #1061).
#[driver_test(requires(scan))]
pub async fn cross_variant_or(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Contact {
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
        contact: Contact,
    }

    let mut db = t.setup_db(models!(User)).await;

    User::create()
        .contact(Contact::Email {
            address: "x".to_string(),
        })
        .exec(&mut db)
        .await?;
    User::create()
        .contact(Contact::Phone {
            number: "x".to_string(),
        })
        .exec(&mut db)
        .await?;

    let rows = User::filter(
        User::fields()
            .contact()
            .email()
            .address()
            .eq("x")
            .or(User::fields().contact().phone().number().eq("x")),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(rows.len(), 2);
    Ok(())
}

/// A variant field whose model type differs from its stored column type
/// (`uuid::Uuid`, stored as a string) is filterable. The decode cast the
/// enum's `Match` arm wraps around the column used to reach the SQL
/// serializer unchanged and panic; simplify now moves the conversion onto
/// the constant side.
#[driver_test]
pub async fn filter_variant_field_with_cast_storage(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Owner {
        #[column(variant = 1)]
        Human { id: uuid::Uuid },
        #[column(variant = 2)]
        Animal { tag: uuid::Uuid },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = t.setup_db(models!(Object)).await;

    let target = uuid::Uuid::new_v4();
    let expected = toasty::create!(Object {
        owner: Owner::Human { id: target }
    })
    .exec(&mut db)
    .await?;
    // Same UUID in the other variant's column: must not match.
    toasty::create!(Object {
        owner: Owner::Animal { tag: target }
    })
    .exec(&mut db)
    .await?;

    let found = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|h| h.id().eq(target)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, expected.id);

    Ok(())
}
