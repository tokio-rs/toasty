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
    .all(&mut db)
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
        .all(&mut db)
        .await?;

    assert_eq!(emails.len(), 2);

    let phones = User::filter(User::fields().contact().is_phone())
        .all(&mut db)
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
        .all(&mut db)
        .await?;
    assert_eq!(active.len(), 2);

    let pending = Task::filter(Task::fields().status().is_pending())
        .all(&mut db)
        .await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].name, "A");

    let done = Task::filter(Task::fields().status().is_done())
        .all(&mut db)
        .await?;
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].name, "D");

    Ok(())
}

/// Filters by enum variant using `is_{variant}()` combined with a partition key.
/// The partition key satisfies DynamoDB's index requirement; the variant check
/// becomes a FilterExpression applied server-side after the key lookup.
#[driver_test]
pub async fn filter_enum_variant_with_partition_key(t: &mut Test) -> Result<()> {
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
    #[key(partition = owner, local = id)]
    #[allow(dead_code)]
    struct Task {
        #[auto]
        id: uuid::Uuid,
        owner: String,
        title: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    for (owner, title, status) in [
        ("alice", "Task A", Status::Pending),
        ("alice", "Task B", Status::Active),
        ("alice", "Task C", Status::Active),
        ("alice", "Task D", Status::Done),
        ("bob", "Task E", Status::Active),
    ] {
        Task::create()
            .owner(owner)
            .title(title)
            .status(status)
            .exec(&mut db)
            .await?;
    }

    // Partition key + variant filter
    let active = Task::filter(
        Task::fields()
            .owner()
            .eq("alice")
            .and(Task::fields().status().is_active()),
    )
    .all(&mut db)
    .await?;

    assert_eq!(active.len(), 2);

    let done = Task::filter(
        Task::fields()
            .owner()
            .eq("alice")
            .and(Task::fields().status().is_done()),
    )
    .all(&mut db)
    .await?;

    assert_eq!(done.len(), 1);
    assert_eq!(done[0].title, "Task D");

    // Bob has one active task
    let bob_active = Task::filter(
        Task::fields()
            .owner()
            .eq("bob")
            .and(Task::fields().status().is_active()),
    )
    .all(&mut db)
    .await?;

    assert_eq!(bob_active.len(), 1);
    assert_eq!(bob_active[0].title, "Task E");

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
