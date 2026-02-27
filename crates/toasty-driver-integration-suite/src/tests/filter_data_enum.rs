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

    let mut db = t.setup_db(models!(User, ContactInfo)).await;

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

    let emails = User::filter(User::fields().contact().eq(ContactInfo::Email {
        address: "alice@example.com".to_string(),
    }))
    .collect::<Vec<_>>(&mut db)
    .await?;

    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].name, "Alice");

    Ok(())
}

/// Filtering by variant alone (discriminant-only check) using `is_{variant}()`.
#[driver_test(requires(sql))]
pub async fn filter_data_enum_by_variant(t: &mut Test) -> Result<()> {
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

    let mut db = t.setup_db(models!(User, ContactInfo)).await;

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

    let emails = User::filter(User::fields().contact().is_email())
        .collect::<Vec<_>>(&mut db)
        .await?;

    assert_eq!(emails.len(), 2);

    let phones = User::filter(User::fields().contact().is_phone())
        .collect::<Vec<_>>(&mut db)
        .await?;

    assert_eq!(phones.len(), 1);
    assert_eq!(phones[0].name, "Bob");

    Ok(())
}

/// Filtering a unit-only enum by variant using `is_{variant}()`.
#[driver_test(requires(sql))]
pub async fn filter_unit_enum_by_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    Task::create()
        .name("A")
        .status(Status::Pending)
        .exec(&mut db)
        .await?;

    Task::create()
        .name("B")
        .status(Status::Active)
        .exec(&mut db)
        .await?;

    Task::create()
        .name("C")
        .status(Status::Active)
        .exec(&mut db)
        .await?;

    Task::create()
        .name("D")
        .status(Status::Done)
        .exec(&mut db)
        .await?;

    let active = Task::filter(Task::fields().status().is_active())
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(active.len(), 2);

    let pending = Task::filter(Task::fields().status().is_pending())
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "A");

    let done = Task::filter(Task::fields().status().is_done())
        .collect::<Vec<_>>(&mut db)
        .await?;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].name, "D");

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

    let mut db = t.setup_db(models!(User, ContactInfo)).await;

    let alice = User::create()
        .name("Alice")
        .contact(ContactInfo::Email {
            address: "alice@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    let bob = User::create()
        .name("Bob")
        .contact(ContactInfo::Phone {
            number: "555-1234".to_string(),
        })
        .exec(&mut db)
        .await?;

    let found_alice = User::get_by_id(&mut db, &alice.id).await?;
    assert_eq!(found_alice.name, "Alice");
    assert_eq!(
        found_alice.contact,
        ContactInfo::Email {
            address: "alice@example.com".to_string()
        }
    );

    let found_bob = User::get_by_id(&mut db, &bob.id).await?;
    assert_eq!(found_bob.name, "Bob");
    assert_eq!(
        found_bob.contact,
        ContactInfo::Phone {
            number: "555-1234".to_string()
        }
    );

    Ok(())
}
