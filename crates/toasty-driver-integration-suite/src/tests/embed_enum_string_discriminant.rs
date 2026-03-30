use crate::prelude::*;

/// Tests basic CRUD with a unit enum using explicit string discriminants.
#[driver_test(id(ID))]
pub async fn string_discriminant_unit_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = "pending")]
        Pending,
        #[column(variant = "active")]
        Active,
        #[column(variant = "done")]
        Done,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: ID,
        title: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    let task = Task::create()
        .title("Ship it")
        .status(Status::Pending)
        .exec(&mut db)
        .await?;
    assert_eq!(task.status, Status::Pending);

    let found = Task::get_by_id(&mut db, &task.id).await?;
    assert_eq!(found.status, Status::Pending);

    // Update and re-read
    let mut task = found;
    task.update().status(Status::Active).exec(&mut db).await?;
    let found = Task::get_by_id(&mut db, &task.id).await?;
    assert_eq!(found.status, Status::Active);

    Ok(())
}

/// Tests unit enum with default labels (variant ident used as string label).
#[driver_test(id(ID))]
pub async fn default_string_labels(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Priority {
        Low,
        Medium,
        High,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: ID,
        title: String,
        priority: Priority,
    }

    let mut db = t.setup_db(models!(Task, Priority)).await;

    let task = Task::create()
        .title("Fix bug")
        .priority(Priority::High)
        .exec(&mut db)
        .await?;
    assert_eq!(task.priority, Priority::High);

    let found = Task::get_by_id(&mut db, &task.id).await?;
    assert_eq!(found.priority, Priority::High);

    Ok(())
}

/// Tests mixing explicit string labels with default labels.
#[driver_test(id(ID))]
pub async fn mixed_explicit_and_default_labels(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = "waiting")]
        Pending,
        Active,
        Done,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: ID,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    // "waiting" is the explicit label for Pending
    let t1 = Task::create().status(Status::Pending).exec(&mut db).await?;
    assert_eq!(t1.status, Status::Pending);

    // "Active" is the default label
    let t2 = Task::create().status(Status::Active).exec(&mut db).await?;

    let found1 = Task::get_by_id(&mut db, &t1.id).await?;
    let found2 = Task::get_by_id(&mut db, &t2.id).await?;
    assert_eq!(found1.status, Status::Pending);
    assert_eq!(found2.status, Status::Active);

    Ok(())
}

/// Tests data-carrying enum with string discriminants.
#[driver_test(id(ID))]
pub async fn string_discriminant_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactMethod {
        #[column(variant = "email")]
        Email { address: String },
        #[column(variant = "phone")]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        contact: ContactMethod,
    }

    let mut db = t.setup_db(models!(User, ContactMethod)).await;

    let user = User::create()
        .name("Alice")
        .contact(ContactMethod::Email {
            address: "alice@example.com".into(),
        })
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(
        found.contact,
        ContactMethod::Email {
            address: "alice@example.com".into()
        }
    );

    // Update to a different variant
    let mut user = found;
    user.update()
        .contact(ContactMethod::Phone {
            number: "555-0100".into(),
        })
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(
        found.contact,
        ContactMethod::Phone {
            number: "555-0100".into()
        }
    );

    Ok(())
}

/// Tests data-carrying enum with default string labels (variant ident as discriminant).
#[driver_test(id(ID))]
pub async fn default_string_labels_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactMethod {
        Email { address: String },
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        contact: ContactMethod,
    }

    let mut db = t.setup_db(models!(User, ContactMethod)).await;

    let user = User::create()
        .name("Alice")
        .contact(ContactMethod::Email {
            address: "alice@example.com".into(),
        })
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(
        found.contact,
        ContactMethod::Email {
            address: "alice@example.com".into()
        }
    );

    // Update to a different variant
    let mut user = found;
    user.update()
        .contact(ContactMethod::Phone {
            number: "555-0100".into(),
        })
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(
        found.contact,
        ContactMethod::Phone {
            number: "555-0100".into()
        }
    );

    Ok(())
}

/// Tests data-carrying enum mixing explicit string labels with defaults.
#[driver_test(id(ID))]
pub async fn mixed_string_labels_data_enum(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactMethod {
        #[column(variant = "mail")]
        Email {
            address: String,
        },
        Phone {
            number: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        contact: ContactMethod,
    }

    let mut db = t.setup_db(models!(User, ContactMethod)).await;

    // Create with the explicit-label variant
    let u1 = User::create()
        .name("Alice")
        .contact(ContactMethod::Email {
            address: "alice@example.com".into(),
        })
        .exec(&mut db)
        .await?;

    // Create with the default-label variant
    let u2 = User::create()
        .name("Bob")
        .contact(ContactMethod::Phone {
            number: "555-0200".into(),
        })
        .exec(&mut db)
        .await?;

    let found1 = User::get_by_id(&mut db, &u1.id).await?;
    assert_eq!(
        found1.contact,
        ContactMethod::Email {
            address: "alice@example.com".into()
        }
    );

    let found2 = User::get_by_id(&mut db, &u2.id).await?;
    assert_eq!(
        found2.contact,
        ContactMethod::Phone {
            number: "555-0200".into()
        }
    );

    // Update from explicit-label variant to default-label variant
    let mut user = found1;
    user.update()
        .contact(ContactMethod::Phone {
            number: "555-0300".into(),
        })
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(
        found.contact,
        ContactMethod::Phone {
            number: "555-0300".into()
        }
    );

    Ok(())
}

/// Tests filtering by variant with string discriminants.
#[driver_test(requires(sql))]
pub async fn filter_by_string_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = "pending")]
        Pending,
        #[column(variant = "active")]
        Active,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
        status: Status,
    }

    let mut db = t.setup_db(models!(Task, Status)).await;

    Task::create()
        .title("A")
        .status(Status::Pending)
        .exec(&mut db)
        .await?;

    Task::create()
        .title("B")
        .status(Status::Active)
        .exec(&mut db)
        .await?;

    let pending = Task::filter(Task::fields().status().is_pending())
        .exec(&mut db)
        .await?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].title, "A");

    Ok(())
}

/// Verifies the schema registers string discriminants with the correct type.
#[driver_test]
pub async fn string_discriminant_schema_registration(t: &mut Test) {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Color {
        #[column(variant = "red")]
        Red,
        #[column(variant = "green")]
        Green,
        #[column(variant = "blue")]
        Blue,
    }

    let db = t.setup_db(models!(Color)).await;
    let schema = db.schema();

    let color_model = schema.app.model(Color::id()).as_embedded_enum_unwrap();
    assert_eq!(color_model.discriminant.ty, toasty_core::stmt::Type::String);
    assert_eq!(color_model.variants.len(), 3);
    assert_eq!(
        color_model.variants[0].discriminant,
        toasty_core::stmt::Value::String("red".to_string())
    );
    assert_eq!(
        color_model.variants[1].discriminant,
        toasty_core::stmt::Value::String("green".to_string())
    );
    assert_eq!(
        color_model.variants[2].discriminant,
        toasty_core::stmt::Value::String("blue".to_string())
    );
}
