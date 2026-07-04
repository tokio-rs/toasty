use crate::prelude::*;

/// Verifies that a data-carrying enum has its variant fields registered in the app
/// schema with globally-assigned field indices (indices are unique across all variants).
#[driver_test(scenario(crate::scenarios::user_contact_info))]
pub async fn data_carrying_enum_schema(t: &mut Test) {
    let db = setup(t).await;
    let schema = db.schema();

    let contact_info = &schema.app.models[&ContactInfo::id()];
    assert_struct!(contact_info, toasty::schema::app::Model::EmbeddedEnum({
        name.upper_camel_case(): "ContactInfo",
        variants: [
            {
                name.upper_camel_case(): "Email",
                discriminant: toasty_core::stmt::Value::I64(1),
                ..
            },
            {
                name.upper_camel_case(): "Phone",
                discriminant: toasty_core::stmt::Value::I64(2),
                ..
            },
        ],
        fields: [
            { id.index: 0, name.app: Some("address") },
            { id.index: 1, name.app: Some("number") },
        ],
    }));
}

/// Verifies that a mixed enum (some unit variants, some data variants) registers
/// correctly: unit variants have empty `fields`, data variants have their fields
/// with indices assigned starting from 0 and continuing globally across variants.
#[driver_test(scenario(crate::scenarios::task_with_status))]
pub async fn mixed_enum_schema(t: &mut Test) {
    let db = setup(t).await;
    let schema = db.schema();

    let status = &schema.app.models[&Status::id()];
    assert_struct!(status, toasty::schema::app::Model::EmbeddedEnum({
        variants: [
            {
                name.upper_camel_case(): "Pending",
                discriminant: toasty_core::stmt::Value::I64(1),
                ..
            },
            {
                name.upper_camel_case(): "Failed",
                discriminant: toasty_core::stmt::Value::I64(2),
                ..
            },
            {
                name.upper_camel_case(): "Done",
                discriminant: toasty_core::stmt::Value::I64(3),
                ..
            },
        ],
        fields: [
            { id.index: 0, name.app: Some("reason") },
        ],
    }));
}

/// Verifies DB columns for a data-carrying enum: discriminant column + one nullable
/// column per variant field, named `{disc_col}_{field_name}`.
#[driver_test]
pub async fn data_carrying_enum_db_schema(test: &mut Test) {
    #[allow(dead_code)]
    #[derive(toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        contact: ContactInfo,
    }

    let db = test.setup_db(models!(User)).await;
    let schema = db.schema();

    // The DB table has disc col + one col per variant field (2 variants × 1 field each).
    assert_struct!(schema.db.tables, [
        {
            name: =~ r"users$",
            columns: [
                { name: "id" },
                { name: "contact", nullable: false },
                { name: "contact_address", nullable: true },
                { name: "contact_number", nullable: true },
            ],
        },
    ]);
}

/// End-to-end CRUD test for a data-carrying enum (all variants have fields).
/// Creates records with different variants, reads them back, and verifies roundtrip.
#[driver_test(scenario(crate::scenarios::user_contact_info))]
pub async fn data_variant_roundtrip(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

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

    // Read back and check values are reconstructed correctly.
    let found_alice = User::get_by_id(&mut db, &alice.id).await?;
    assert_eq!(
        found_alice.contact,
        ContactInfo::Email {
            address: "alice@example.com".to_string()
        }
    );

    let found_bob = User::get_by_id(&mut db, &bob.id).await?;
    assert_eq!(
        found_bob.contact,
        ContactInfo::Phone {
            number: "555-1234".to_string()
        }
    );

    // Clean up.
    alice.delete().exec(&mut db).await?;
    bob.delete().exec(&mut db).await?;
    Ok(())
}

/// End-to-end CRUD test for a mixed enum (unit variants and data variants).
/// Verifies that both kinds round-trip correctly through the DB.
#[driver_test(scenario(crate::scenarios::task_with_status))]
pub async fn mixed_enum_roundtrip(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let pending = Task::create()
        .title("Pending task")
        .status(Status::Pending)
        .exec(&mut db)
        .await?;

    let failed = Task::create()
        .title("Failed task")
        .status(Status::Failed {
            reason: "out of memory".to_string(),
        })
        .exec(&mut db)
        .await?;

    let done = Task::create()
        .title("Done task")
        .status(Status::Done)
        .exec(&mut db)
        .await?;

    let found_pending = Task::get_by_id(&mut db, &pending.id).await?;
    assert_eq!(found_pending.status, Status::Pending);

    let found_failed = Task::get_by_id(&mut db, &failed.id).await?;
    assert_eq!(
        found_failed.status,
        Status::Failed {
            reason: "out of memory".to_string()
        }
    );

    let found_done = Task::get_by_id(&mut db, &done.id).await?;
    assert_eq!(found_done.status, Status::Done);

    Ok(())
}

/// Updating a mixed enum field from a data-carrying variant to a unit variant
/// (and back) round-trips correctly. Regression test for #1068: the unit
/// variant's value record is narrower than the data variant's data column
/// expects, which used to panic while lowering the update.
#[driver_test(scenario(crate::scenarios::task_with_status))]
pub async fn mixed_enum_update_data_to_unit_variant(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut task = toasty::create!(Task {
        title: "task",
        status: Status::Failed {
            reason: "boom".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    // Data variant -> unit variant (the reported panic).
    task.update().status(Status::Pending).exec(&mut db).await?;
    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.status,
        Status::Pending
    );

    // Unit variant -> data variant, to confirm the reverse still works.
    task.update()
        .status(Status::Failed {
            reason: "again".to_string(),
        })
        .exec(&mut db)
        .await?;
    assert_eq!(
        Task::get_by_id(&mut db, &task.id).await?.status,
        Status::Failed {
            reason: "again".to_string()
        }
    );

    Ok(())
}

/// Updating a data-carrying enum between variants with *different field counts*
/// round-trips. The narrower variant's value record is shorter than the wider
/// variant's data columns expect, exercising the same out-of-bounds projection
/// path as #1068 without any unit variant involved.
#[driver_test]
pub async fn enum_update_between_variants_of_different_width(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Event {
        #[column(variant = 1)]
        Login { user: String },
        #[column(variant = 2)]
        Purchase { item: String, amount: i64 },
    }

    #[derive(Debug, toasty::Model)]
    struct Log {
        #[key]
        #[auto]
        id: uuid::Uuid,
        event: Event,
    }

    let mut db = test.setup_db(models!(Log)).await;

    let mut log = toasty::create!(Log {
        event: Event::Purchase {
            item: "book".to_string(),
            amount: 42,
        },
    })
    .exec(&mut db)
    .await?;

    // Wider variant -> narrower variant.
    log.update()
        .event(Event::Login {
            user: "alice".to_string(),
        })
        .exec(&mut db)
        .await?;
    assert_eq!(
        Log::get_by_id(&mut db, &log.id).await?.event,
        Event::Login {
            user: "alice".to_string()
        }
    );

    Ok(())
}

/// Tests that UUID fields inside data-carrying enum variants round-trip correctly.
/// UUID is a non-trivial primitive that requires type casting on some databases.
#[driver_test]
pub async fn data_variant_with_uuid_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum OrderRef {
        #[column(variant = 1)]
        Internal { id: uuid::Uuid },
        #[column(variant = 2)]
        External { code: String },
    }

    #[derive(Debug, toasty::Model)]
    struct Order {
        #[key]
        #[auto]
        id: uuid::Uuid,
        order_ref: OrderRef,
    }

    let mut db = test.setup_db(models!(Order)).await;

    let internal_id = uuid::Uuid::new_v4();

    let o1 = Order::create()
        .order_ref(OrderRef::Internal { id: internal_id })
        .exec(&mut db)
        .await?;

    let o2 = Order::create()
        .order_ref(OrderRef::External {
            code: "EXT-001".to_string(),
        })
        .exec(&mut db)
        .await?;

    let found_o1 = Order::get_by_id(&mut db, &o1.id).await?;
    assert_eq!(found_o1.order_ref, OrderRef::Internal { id: internal_id });

    let found_o2 = Order::get_by_id(&mut db, &o2.id).await?;
    assert_eq!(
        found_o2.order_ref,
        OrderRef::External {
            code: "EXT-001".to_string()
        }
    );

    Ok(())
}

/// Tests that jiff::Timestamp fields inside data-carrying enum variants round-trip correctly.
/// Also covers a mixed enum (one unit variant, one data variant) to verify null handling.
#[driver_test]
pub async fn data_variant_with_jiff_timestamp(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum EventTime {
        #[column(variant = 1)]
        Scheduled { at: jiff::Timestamp },
        #[column(variant = 2)]
        Unscheduled,
    }

    #[derive(Debug, toasty::Model)]
    struct Event {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        time: EventTime,
    }

    let mut db = test.setup_db(models!(Event)).await;

    let ts = jiff::Timestamp::from_second(1_700_000_000).unwrap();

    let scheduled = Event::create()
        .name("launch")
        .time(EventTime::Scheduled { at: ts })
        .exec(&mut db)
        .await?;

    let unscheduled = Event::create()
        .name("tbd")
        .time(EventTime::Unscheduled)
        .exec(&mut db)
        .await?;

    let found_scheduled = Event::get_by_id(&mut db, &scheduled.id).await?;
    assert_eq!(found_scheduled.time, EventTime::Scheduled { at: ts });

    let found_unscheduled = Event::get_by_id(&mut db, &unscheduled.id).await?;
    assert_eq!(found_unscheduled.time, EventTime::Unscheduled);

    Ok(())
}

#[driver_test]
pub async fn struct_in_data_variant(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Destination {
        #[column(variant = 1)]
        Digital { email: String },
        #[column(variant = 2)]
        Physical { address: Address },
    }

    #[derive(Debug, toasty::Model)]
    struct Shipment {
        #[key]
        #[auto]
        id: uuid::Uuid,
        destination: Destination,
    }

    let mut db = test.setup_db(models!(Shipment)).await;

    let digital = Shipment::create()
        .destination(Destination::Digital {
            email: "user@example.com".to_string(),
        })
        .exec(&mut db)
        .await?;

    let physical = Shipment::create()
        .destination(Destination::Physical {
            address: Address {
                street: "123 Main St".to_string(),
                city: "Seattle".to_string(),
            },
        })
        .exec(&mut db)
        .await?;

    let found_digital = Shipment::get_by_id(&mut db, &digital.id).await?;
    assert_eq!(
        found_digital.destination,
        Destination::Digital {
            email: "user@example.com".to_string()
        }
    );

    let found_physical = Shipment::get_by_id(&mut db, &physical.id).await?;
    assert_eq!(
        found_physical.destination,
        Destination::Physical {
            address: Address {
                street: "123 Main St".to_string(),
                city: "Seattle".to_string(),
            },
        }
    );

    Ok(())
}

/// Roundtrip test for an enum embedded inside a variant field of another enum (enum-in-enum).
/// The inner enum is unit-only; the outer has one data variant and one unit variant.
#[driver_test]
pub async fn enum_in_enum_roundtrip(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Channel {
        #[column(variant = 1)]
        Email,
        #[column(variant = 2)]
        Sms,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Notification {
        #[column(variant = 1)]
        Send { channel: Channel, message: String },
        #[column(variant = 2)]
        Suppress,
    }

    #[derive(Debug, toasty::Model)]
    struct Alert {
        #[key]
        #[auto]
        id: uuid::Uuid,
        notification: Notification,
    }

    let mut db = test.setup_db(models!(Alert)).await;

    let a1 = Alert::create()
        .notification(Notification::Send {
            channel: Channel::Email,
            message: "hello".to_string(),
        })
        .exec(&mut db)
        .await?;

    let a2 = Alert::create()
        .notification(Notification::Send {
            channel: Channel::Sms,
            message: "world".to_string(),
        })
        .exec(&mut db)
        .await?;

    let a3 = Alert::create()
        .notification(Notification::Suppress)
        .exec(&mut db)
        .await?;

    let found_a1 = Alert::get_by_id(&mut db, &a1.id).await?;
    assert_eq!(
        found_a1.notification,
        Notification::Send {
            channel: Channel::Email,
            message: "hello".to_string(),
        }
    );

    let found_a2 = Alert::get_by_id(&mut db, &a2.id).await?;
    assert_eq!(
        found_a2.notification,
        Notification::Send {
            channel: Channel::Sms,
            message: "world".to_string(),
        }
    );

    let found_a3 = Alert::get_by_id(&mut db, &a3.id).await?;
    assert_eq!(found_a3.notification, Notification::Suppress);

    Ok(())
}

/// Verifies field indices are assigned globally across multiple data variants.
/// With two variants having two fields each, indices should be 0, 1, 2, 3.
#[driver_test]
pub async fn global_field_indices(test: &mut Test) {
    #[allow(dead_code)]
    #[derive(toasty::Embed)]
    enum Event {
        #[column(variant = 1)]
        Login { user_id: String, ip: String },
        #[column(variant = 2)]
        Purchase { item_id: String, amount: i64 },
    }

    #[derive(toasty::Model)]
    #[allow(dead_code)]
    struct Container {
        #[key]
        id: i64,
        event: Event,
    }

    let db = test.setup_db(models!(Container)).await;
    let schema = db.schema();

    let event = &schema.app.models[&Event::id()];
    assert_struct!(event, toasty::schema::app::Model::EmbeddedEnum({
        fields: [
            { id.index: 0, name.app: Some("user_id") },
            { id.index: 1, name.app: Some("ip") },
            { id.index: 2, name.app: Some("item_id") },
            { id.index: 3, name.app: Some("amount") },
        ],
    }));
}
