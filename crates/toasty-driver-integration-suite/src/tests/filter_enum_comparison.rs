use crate::prelude::*;

/// Filters unit enum field using `ne()`.
#[driver_test(requires(sql))]
pub async fn filter_unit_enum_ne(t: &mut Test) -> Result<()> {
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

    for (name, status) in [
        ("A", Status::Pending),
        ("B", Status::Active),
        ("C", Status::Active),
        ("D", Status::Done),
    ] {
        Task::create()
            .name(name)
            .status(status)
            .exec(&mut db)
            .await?;
    }

    // ne(Active) should return Pending and Done
    let not_active = Task::filter(Task::fields().status().ne(Status::Active))
        .all(&mut db)
        .await?;

    assert_eq!(not_active.len(), 2);
    assert_eq_unordered!(not_active.iter().map(|t| &*t.name), ["A", "D"]);

    Ok(())
}

/// Filters unit enum field using `in_list()`.
#[driver_test(requires(sql))]
pub async fn filter_unit_enum_in_list(t: &mut Test) -> Result<()> {
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

    for (name, status) in [
        ("A", Status::Pending),
        ("B", Status::Active),
        ("C", Status::Active),
        ("D", Status::Done),
    ] {
        Task::create()
            .name(name)
            .status(status)
            .exec(&mut db)
            .await?;
    }

    // in_list([Pending, Done]) should return A and D
    let pending_or_done = Task::filter(
        Task::fields()
            .status()
            .in_list([Status::Pending, Status::Done]),
    )
    .all(&mut db)
    .await?;

    assert_eq!(pending_or_done.len(), 2);
    assert_eq_unordered!(pending_or_done.iter().map(|t| &*t.name), ["A", "D"]);

    Ok(())
}

/// Filters data-carrying enum field using `ne()`.
#[driver_test(requires(sql))]
pub async fn filter_data_enum_ne(t: &mut Test) -> Result<()> {
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

    // ne(Alice's email) should return Bob
    let not_alice_email = User::filter(User::fields().contact().ne(ContactInfo::Email {
        address: "alice@example.com".to_string(),
    }))
    .all(&mut db)
    .await?;

    assert_eq!(not_alice_email.len(), 1);
    assert_eq!(not_alice_email[0].name, "Bob");

    Ok(())
}
