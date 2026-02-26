use crate::prelude::*;

/// Verifies that a data-carrying enum has its variant fields registered in the app
/// schema with globally-assigned field indices (indices are unique across all variants).
#[driver_test]
pub async fn data_carrying_enum_schema(test: &mut Test) {
    #[allow(dead_code)]
    #[derive(toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    let db = test.setup_db(models!(ContactInfo)).await;
    let schema = db.schema();

    assert_struct!(schema.app.models, #{
        ContactInfo::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            name.upper_camel_case(): "ContactInfo",
            variants: [
                _ {
                    name.upper_camel_case(): "Email",
                    discriminant: 1,
                    fields: [
                        _ { id.index: 0, name.app_name: "address", .. },
                    ],
                    ..
                },
                _ {
                    name.upper_camel_case(): "Phone",
                    discriminant: 2,
                    fields: [
                        _ { id.index: 1, name.app_name: "number", .. },
                    ],
                    ..
                },
            ],
            ..
        }),
    });
}

/// Verifies that a mixed enum (some unit variants, some data variants) registers
/// correctly: unit variants have empty `fields`, data variants have their fields
/// with indices assigned starting from 0 and continuing globally across variants.
#[driver_test]
pub async fn mixed_enum_schema(test: &mut Test) {
    #[allow(dead_code)]
    #[derive(toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Failed { reason: String },
        #[column(variant = 3)]
        Done,
    }

    let db = test.setup_db(models!(Status)).await;
    let schema = db.schema();

    assert_struct!(schema.app.models, #{
        Status::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            variants: [
                _ {
                    name.upper_camel_case(): "Pending",
                    discriminant: 1,
                    fields.len(): 0,
                    ..
                },
                _ {
                    name.upper_camel_case(): "Failed",
                    discriminant: 2,
                    fields: [
                        _ { id.index: 0, name.app_name: "reason", .. },
                    ],
                    ..
                },
                _ {
                    name.upper_camel_case(): "Done",
                    discriminant: 3,
                    fields.len(): 0,
                    ..
                },
            ],
            ..
        }),
    });
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

    let db = test.setup_db(models!(User, ContactInfo)).await;
    let schema = db.schema();

    // The DB table has disc col + one col per variant field (2 variants × 1 field each).
    assert_struct!(schema.db.tables, [
        _ {
            name: =~ r"users$",
            columns: [
                _ { name: "id", .. },
                _ { name: "contact", nullable: false, .. },
                _ { name: "contact_address", nullable: true, .. },
                _ { name: "contact_number", nullable: true, .. },
            ],
            ..
        }
    ]);
}

/// End-to-end CRUD test for a data-carrying enum (all variants have fields).
/// Creates records with different variants, reads them back, and verifies roundtrip.
#[driver_test]
pub async fn data_variant_roundtrip(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum ContactInfo {
        #[column(variant = 1)]
        Email { address: String },
        #[column(variant = 2)]
        Phone { number: String },
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        contact: ContactInfo,
    }

    let db = test.setup_db(models!(User, ContactInfo)).await;

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

    // Read back and check values are reconstructed correctly.
    let found_alice = User::get_by_id(&db, &alice.id).await?;
    assert_eq!(
        found_alice.contact,
        ContactInfo::Email {
            address: "alice@example.com".to_string()
        }
    );

    let found_bob = User::get_by_id(&db, &bob.id).await?;
    assert_eq!(
        found_bob.contact,
        ContactInfo::Phone {
            number: "555-1234".to_string()
        }
    );

    // Clean up.
    alice.delete(&db).await?;
    bob.delete(&db).await?;
    Ok(())
}

/// End-to-end CRUD test for a mixed enum (unit variants and data variants).
/// Verifies that both kinds round-trip correctly through the DB.
#[driver_test]
pub async fn mixed_enum_roundtrip(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Failed { reason: String },
        #[column(variant = 3)]
        Done,
    }

    #[derive(Debug, toasty::Model)]
    struct Task {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
        status: Status,
    }

    let db = test.setup_db(models!(Task, Status)).await;

    let pending = Task::create()
        .title("Pending task")
        .status(Status::Pending)
        .exec(&db)
        .await?;

    let failed = Task::create()
        .title("Failed task")
        .status(Status::Failed {
            reason: "out of memory".to_string(),
        })
        .exec(&db)
        .await?;

    let done = Task::create()
        .title("Done task")
        .status(Status::Done)
        .exec(&db)
        .await?;

    let found_pending = Task::get_by_id(&db, &pending.id).await?;
    assert_eq!(found_pending.status, Status::Pending);

    let found_failed = Task::get_by_id(&db, &failed.id).await?;
    assert_eq!(
        found_failed.status,
        Status::Failed {
            reason: "out of memory".to_string()
        }
    );

    let found_done = Task::get_by_id(&db, &done.id).await?;
    assert_eq!(found_done.status, Status::Done);

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

    let db = test.setup_db(models!(Order, OrderRef)).await;

    let internal_id = uuid::Uuid::new_v4();

    let o1 = Order::create()
        .order_ref(OrderRef::Internal { id: internal_id })
        .exec(&db)
        .await?;

    let o2 = Order::create()
        .order_ref(OrderRef::External {
            code: "EXT-001".to_string(),
        })
        .exec(&db)
        .await?;

    let found_o1 = Order::get_by_id(&db, &o1.id).await?;
    assert_eq!(found_o1.order_ref, OrderRef::Internal { id: internal_id });

    let found_o2 = Order::get_by_id(&db, &o2.id).await?;
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

    let db = test.setup_db(models!(Event, EventTime)).await;

    let ts = jiff::Timestamp::from_second(1_700_000_000).unwrap();

    let scheduled = Event::create()
        .name("launch")
        .time(EventTime::Scheduled { at: ts })
        .exec(&db)
        .await?;

    let unscheduled = Event::create()
        .name("tbd")
        .time(EventTime::Unscheduled)
        .exec(&db)
        .await?;

    let found_scheduled = Event::get_by_id(&db, &scheduled.id).await?;
    assert_eq!(found_scheduled.time, EventTime::Scheduled { at: ts });

    let found_unscheduled = Event::get_by_id(&db, &unscheduled.id).await?;
    assert_eq!(found_unscheduled.time, EventTime::Unscheduled);

    Ok(())
}

// TODO: struct-in-enum flattening not yet implemented (struct-typed variant fields
// need recursive column expansion in the schema builder).
#[ignore]
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

    let db = test.setup_db(models!(Shipment, Destination, Address)).await;

    let digital = Shipment::create()
        .destination(Destination::Digital {
            email: "user@example.com".to_string(),
        })
        .exec(&db)
        .await?;

    let physical = Shipment::create()
        .destination(Destination::Physical {
            address: Address {
                street: "123 Main St".to_string(),
                city: "Seattle".to_string(),
            },
        })
        .exec(&db)
        .await?;

    let found_digital = Shipment::get_by_id(&db, &digital.id).await?;
    assert_eq!(
        found_digital.destination,
        Destination::Digital {
            email: "user@example.com".to_string()
        }
    );

    let found_physical = Shipment::get_by_id(&db, &physical.id).await?;
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

    let db = test.setup_db(models!(Alert, Notification, Channel)).await;

    let a1 = Alert::create()
        .notification(Notification::Send {
            channel: Channel::Email,
            message: "hello".to_string(),
        })
        .exec(&db)
        .await?;

    let a2 = Alert::create()
        .notification(Notification::Send {
            channel: Channel::Sms,
            message: "world".to_string(),
        })
        .exec(&db)
        .await?;

    let a3 = Alert::create()
        .notification(Notification::Suppress)
        .exec(&db)
        .await?;

    let found_a1 = Alert::get_by_id(&db, &a1.id).await?;
    assert_eq!(
        found_a1.notification,
        Notification::Send {
            channel: Channel::Email,
            message: "hello".to_string(),
        }
    );

    let found_a2 = Alert::get_by_id(&db, &a2.id).await?;
    assert_eq!(
        found_a2.notification,
        Notification::Send {
            channel: Channel::Sms,
            message: "world".to_string(),
        }
    );

    let found_a3 = Alert::get_by_id(&db, &a3.id).await?;
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

    let db = test.setup_db(models!(Event)).await;
    let schema = db.schema();

    assert_struct!(schema.app.models, #{
        Event::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            variants: [
                _ {
                    fields: [
                        _ { id.index: 0, name.app_name: "user_id", .. },
                        _ { id.index: 1, name.app_name: "ip", .. },
                    ],
                    ..
                },
                _ {
                    fields: [
                        _ { id.index: 2, name.app_name: "item_id", .. },
                        _ { id.index: 3, name.app_name: "amount", .. },
                    ],
                    ..
                },
            ],
            ..
        }),
    });
}
