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

    let db = test.setup_db(models!(User)).await;
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

    let mut db = test.setup_db(models!(User)).await;

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

/// Regression test for #973: a unit (data-less) embedded enum used as a model
/// field can be indexed. The index targets the enum's discriminant column.
///
/// Before the fix, building the schema panicked because the index-column
/// resolver had no case for enum mappings.
#[driver_test]
pub async fn unit_enum_field_index_schema(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    enum EntityType {
        Fund,
        Account,
        Department,
        Program,
    }

    #[derive(Debug, toasty::Model)]
    #[index(entity_type)]
    struct Registry {
        #[key]
        id: String,
        #[allow(dead_code)]
        entity_type: EntityType,
    }

    let db = test.setup_db(models!(Registry)).await;
    let schema = db.schema();

    // The enum field is stored as a single discriminant column; the index
    // targets that column.
    let table = &schema.db.tables[0];
    let entity_type_col = table
        .columns
        .iter()
        .find(|c| c.name == "entity_type")
        .expect("entity_type discriminant column should exist");

    // Index 0: primary key (id). Index 1: non-unique on the discriminant column.
    assert_struct!(table.indices, [
        { primary_key: true },
        { unique: false, primary_key: false, columns: [{ column: == entity_type_col.id }] },
    ]);
}

/// Regression test for #973: queries filtering on an indexed unit enum field
/// return the correct rows across all drivers.
#[driver_test]
pub async fn unit_enum_field_index_filter(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum EntityType {
        Fund,
        Account,
        Department,
    }

    #[derive(Debug, toasty::Model)]
    #[index(entity_type)]
    struct Registry {
        #[key]
        id: String,
        entity_type: EntityType,
    }

    let mut db = test.setup_db(models!(Registry)).await;

    toasty::create!(Registry::[
        { id: "1", entity_type: EntityType::Fund },
        { id: "2", entity_type: EntityType::Account },
        { id: "3", entity_type: EntityType::Fund },
        { id: "4", entity_type: EntityType::Department },
    ])
    .exec(&mut db)
    .await?;

    let mut funds = Registry::filter(Registry::fields().entity_type().eq(EntityType::Fund))
        .exec(&mut db)
        .await?;
    funds.sort_by(|a, b| a.id.cmp(&b.id));

    assert_struct!(funds, [{ id: "1" }, { id: "3" }]);

    Ok(())
}

/// Regression test for #973: a unit embedded enum field can participate in a
/// composite `#[unique(...)]` constraint alongside scalar fields, mirroring the
/// `#[unique(dimension_type_id, entity_type, entity_id)]` case from the issue.
///
/// DynamoDB does not support composite unique indices (see
/// `composite_unique_index_unsupported_on_dynamodb` in `index_composite`), so
/// this is SQL-only.
#[driver_test(requires(sql))]
pub async fn unit_enum_composite_unique_enforced(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum EntityType {
        Fund,
        Account,
    }

    #[derive(Debug, toasty::Model)]
    #[unique(entity_type, entity_id)]
    struct Registry {
        #[key]
        #[auto]
        id: u64,
        entity_type: EntityType,
        entity_id: String,
    }

    let mut db = test.setup_db(models!(Registry)).await;

    // The non-primary-key index is unique and spans both columns (the enum's
    // discriminant column plus entity_id).
    let index = db.schema().db.tables[0]
        .indices
        .iter()
        .find(|i| !i.primary_key)
        .expect("composite unique index");
    assert!(index.unique);
    assert_eq!(index.columns.len(), 2);

    toasty::create!(Registry {
        entity_type: EntityType::Fund,
        entity_id: "x"
    })
    .exec(&mut db)
    .await?;

    // The same (entity_type, entity_id) combination is rejected.
    assert_err!(
        toasty::create!(Registry {
            entity_type: EntityType::Fund,
            entity_id: "x"
        })
        .exec(&mut db)
        .await
    );

    // Differing in either column is allowed — uniqueness is on the combination.
    toasty::create!(Registry {
        entity_type: EntityType::Account,
        entity_id: "x"
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Registry {
        entity_type: EntityType::Fund,
        entity_id: "y"
    })
    .exec(&mut db)
    .await?;

    Ok(())
}
