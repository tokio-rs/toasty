use toasty::schema::{
    app::FieldTy,
    mapping::{self, FieldEmbedded, FieldPrimitive},
};
use toasty_core::stmt;

use crate::prelude::*;

/// Tests that embedded structs are registered in the app schema but don't create
/// their own database tables (they're inlined into parent models).
#[driver_test]
pub async fn basic_embedded_struct(test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    let db = test.setup_db(models!(Address)).await;
    let schema = db.schema();

    // Embedded models exist in app schema with ModelKind::Embedded
    assert_struct!(schema.app.models, #{
        Address::id(): _ {
            name.upper_camel_case(): "Address",
            kind: toasty::schema::app::ModelKind::Embedded,
            fields: [
                _ { name.app_name: "street", .. },
                _ { name.app_name: "city", .. }
            ],
            ..
        },
    });

    // Embedded models don't create database tables (fields are flattened into parent)
    assert!(schema.db.tables.is_empty());
}

/// Tests the complete schema generation and mapping for embedded fields:
/// - App schema: embedded field with correct type reference
/// - DB schema: embedded fields flattened to columns (address_street, address_city)
/// - Mapping: projection expressions for field lowering/lifting
#[driver_test]
pub async fn root_model_with_embedded_field(test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        address: Address,
    }

    let db = test.setup_db(models!(User, Address)).await;
    let schema = db.schema();

    // Both embedded and root models exist in app schema
    assert_struct!(schema.app.models, #{
        Address::id(): _ {
            name.upper_camel_case(): "Address",
            kind: toasty::schema::app::ModelKind::Embedded,
            fields: [
                _ { name.app_name: "street", .. },
                _ { name.app_name: "city", .. }
            ],
            ..
        },
        User::id(): _ {
            name.upper_camel_case(): "User",
            kind: toasty::schema::app::ModelKind::Root(_),
            fields: [
                _ { name.app_name: "id", .. },
                _ {
                    name.app_name: "address",
                    ty: FieldTy::Embedded(_ {
                        target: == Address::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        },
    });

    // Database table has flattened columns with prefix (address_street, address_city)
    // This is the key transformation: embedded struct fields become individual columns
    assert_struct!(schema.db.tables, [
        _ {
            name: =~ r"users$",
            columns: [
                _ { name: "id", .. },
                _ { name: "address_street", .. },
                _ { name: "address_city", .. },
            ],
            ..
        }
    ]);

    let user = &schema.app.models[&User::id()];
    let user_table = schema.table_for(user);
    let user_mapping = &schema.mapping.models[&User::id()];

    // Mapping contains projection expressions that extract embedded fields
    // Model -> Table (lowering): project(address_field, [0]) extracts street
    // This allows queries like User.address.city to become address_city column refs
    assert_struct!(user_mapping, _ {
        columns.len(): 3,
        fields: [
            mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[0].id,
                lowering: 0,
                ..
            }),
            mapping::Field::Embedded(FieldEmbedded {
                fields: [
                    mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[1].id,
                        lowering: 1,
                        ..
                    }),
                    mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[2].id,
                        lowering: 2,
                        ..
                    })
                ],
                ..
            }),
        ],
        model_to_table.fields: [
            _,
            == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [0]),
            == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1])
        ],
        ..
    });

    // Table -> Model (lifting): columns are grouped back into record
    // [id_col, street_col, city_col] -> [id, record([street_col, city_col])]
    let table_to_model = user_mapping
        .table_to_model
        .lower_returning_model()
        .into_record();

    assert_struct!(
        table_to_model.fields,
        [
            _,
            stmt::Expr::Record(stmt::ExprRecord { fields: [
                == stmt::Expr::column(user_table.columns[1].id),
                == stmt::Expr::column(user_table.columns[2].id),
            ]}),
        ]
    );
}

/// Tests basic CRUD operations with embedded fields across all ID types.
/// Validates create, read, update (both instance and query-based), and delete.
#[driver_test(id(ID))]
pub async fn create_and_query_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        address: Address,
    }

    let db = t.setup_db(models!(User, Address)).await;

    let mut user = User::create()
        .name("Alice")
        .address(Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
        })
        .exec(&db)
        .await?;

    // Read: embedded struct is reconstructed from flattened columns
    let found = User::get_by_id(&db, &user.id).await?;
    assert_eq!(found.address.street, "123 Main St");
    assert_eq!(found.address.city, "Springfield");

    // Update (instance): entire embedded struct can be replaced
    user.update()
        .address(Address {
            street: "456 Oak Ave".to_string(),
            city: "Shelbyville".to_string(),
        })
        .exec(&db)
        .await?;

    let found = User::get_by_id(&db, &user.id).await?;
    assert_eq!(found.address.street, "456 Oak Ave");

    // Update (query-based): tests query builder with embedded fields
    User::filter_by_id(user.id)
        .update()
        .address(Address {
            street: "789 Pine Rd".to_string(),
            city: "Capital City".to_string(),
        })
        .exec(&db)
        .await?;

    let found = User::get_by_id(&db, &user.id).await?;
    assert_eq!(found.address.street, "789 Pine Rd");

    // Delete: cleanup
    let id = user.id;
    user.delete(&db).await?;
    assert_err!(User::get_by_id(&db, &id).await);
    Ok(())
}

/// Tests code generation for embedded struct field accessors:
/// - User::fields().address() returns AddressFields
/// - Chaining works: User::fields().address().city()
/// - Both model and embedded struct have fields() methods
/// This is purely a compile-time test validating the generated API.
#[driver_test]
pub async fn embedded_struct_fields_codegen(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        address: Address,
    }

    let _db = test.setup_db(models!(User, Address)).await;

    // Direct chaining: User::fields().address().city()
    let _city_path = User::fields().address().city();

    // Intermediate variable: AddressFields can be stored and reused
    let address_fields = User::fields().address();
    let _city_path_2 = address_fields.city();

    // Embedded struct has its own fields() method
    let _address_city = Address::fields().city();

    // Paths are usable in filter expressions (compile-time type check)
    let _query = User::all().filter(User::fields().address().city().eq("Seattle"));
}

/// Tests querying by embedded struct fields with composite keys (DynamoDB compatible).
/// Validates:
/// - Equality queries on embedded fields work across all databases
/// - Different embedded fields (city, zip) can be queried
/// - Multiple partition keys work correctly
/// - Results are properly filtered and returned
#[driver_test]
pub async fn query_embedded_struct_fields(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = country, local = id)]
    #[allow(dead_code)]
    struct User {
        #[auto]
        id: uuid::Uuid,
        country: String,
        name: String,
        address: Address,
    }

    let db = t.setup_db(models!(User, Address)).await;

    // Create users in different countries and cities
    let users_data = [
        ("USA", "Alice", "123 Main St", "Seattle", "98101"),
        ("USA", "Bob", "456 Oak Ave", "Seattle", "98102"),
        ("USA", "Charlie", "789 Pine Rd", "Portland", "97201"),
        ("USA", "Diana", "321 Elm St", "Portland", "97202"),
        ("CAN", "Eve", "111 Maple Dr", "Vancouver", "V6B 1A1"),
        ("CAN", "Frank", "222 Cedar Ln", "Vancouver", "V6B 2B2"),
        ("CAN", "Grace", "333 Birch Way", "Toronto", "M5H 1A1"),
    ];

    for (country, name, street, city, zip) in users_data {
        User::create()
            .country(country)
            .name(name)
            .address(Address {
                street: street.to_string(),
                city: city.to_string(),
                zip: zip.to_string(),
            })
            .exec(&db)
            .await?;
    }

    // Verification: all 7 users were created (DynamoDB requires partition key in queries)
    let mut all_users = Vec::new();
    for country in ["USA", "CAN"] {
        let mut users = User::filter(User::fields().country().eq(country))
            .collect::<Vec<_>>(&db)
            .await?;
        all_users.append(&mut users);
    }
    assert_eq!(all_users.len(), 7);

    // Core test: query by partition key + embedded field
    // This tests the projection simplification: address.city -> address_city column
    let seattle_users = User::filter(
        User::fields()
            .country()
            .eq("USA")
            .and(User::fields().address().city().eq("Seattle")),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(seattle_users.len(), 2);
    let mut names: Vec<_> = seattle_users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Alice", "Bob"]);

    // Validate different partition key (CAN) works
    let vancouver_users = User::filter(
        User::fields()
            .country()
            .eq("CAN")
            .and(User::fields().address().city().eq("Vancouver")),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(vancouver_users.len(), 2);

    // Validate different embedded field (zip instead of city) works
    let user_98101 = User::filter(
        User::fields()
            .country()
            .eq("USA")
            .and(User::fields().address().zip().eq("98101")),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(user_98101.len(), 1);
    assert_eq!(user_98101[0].name, "Alice");
    Ok(())
}

/// Tests comparison operators (gt, lt, ge, le, ne) on embedded struct fields.
/// SQL-only: DynamoDB doesn't support range queries on non-key attributes.
/// Validates that all comparison operators work correctly with embedded fields.
#[driver_test]
pub async fn query_embedded_fields_comparison_ops(t: &mut Test) -> Result<()> {
    // Skip on non-SQL databases (DynamoDB doesn't support range queries on non-key attributes)
    if !t.capability().sql {
        return Ok(());
    }
    #[derive(Debug, toasty::Embed)]
    struct Stats {
        score: i64,
        rank: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Player {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        stats: Stats,
    }

    let db = t.setup_db(models!(Player, Stats)).await;

    for (name, score, rank) in [
        ("Alice", 100, 1),
        ("Bob", 85, 2),
        ("Charlie", 70, 3),
        ("Diana", 55, 4),
        ("Eve", 40, 5),
    ] {
        Player::create()
            .name(name)
            .stats(Stats { score, rank })
            .exec(&db)
            .await?;
    }

    // Test gt: score > 80 should return Alice (100) and Bob (85)
    let high_scorers = Player::filter(Player::fields().stats().score().gt(80))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(high_scorers.len(), 2);

    // Test le: score <= 55 should return Diana (55) and Eve (40)
    let low_scorers = Player::filter(Player::fields().stats().score().le(55))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(low_scorers.len(), 2);

    // Test ne: score != 70 excludes only Charlie
    let not_charlie = Player::filter(Player::fields().stats().score().ne(70))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(not_charlie.len(), 4);

    // Test ge: score >= 70 should return Alice, Bob, Charlie
    let mid_to_high = Player::filter(Player::fields().stats().score().ge(70))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(mid_to_high.len(), 3);
    Ok(())
}

/// Tests querying by multiple embedded fields in a single query (AND conditions).
/// SQL-only: DynamoDB requires partition key in queries.
/// Validates that complex filters with multiple embedded fields work correctly.
#[driver_test]
pub async fn query_embedded_multiple_fields(t: &mut Test) -> Result<()> {
    // Skip on non-SQL databases (DynamoDB requires partition key in queries)
    if !t.capability().sql {
        return Ok(());
    }
    #[derive(Debug, toasty::Embed)]
    struct Coordinates {
        x: i64,
        y: i64,
        z: i64,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Location {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        coords: Coordinates,
    }

    let db = t.setup_db(models!(Location, Coordinates)).await;

    for (name, x, y, z) in [
        ("Origin", 0, 0, 0),
        ("Point A", 10, 20, 0),
        ("Point B", 10, 30, 0),
        ("Point C", 10, 20, 5),
        ("Point D", 20, 20, 0),
    ] {
        Location::create()
            .name(name)
            .coords(Coordinates { x, y, z })
            .exec(&db)
            .await?;
    }

    // Test 2-field AND: x=10 AND y=20 matches Point A (10,20,0) and Point C (10,20,5)
    let matching = Location::filter(
        Location::fields()
            .coords()
            .x()
            .eq(10)
            .and(Location::fields().coords().y().eq(20)),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(matching.len(), 2);
    let mut names: Vec<_> = matching.iter().map(|l| l.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Point A", "Point C"]);

    // Test 3-field AND: adding z=0 narrows to just Point A
    // Validates chaining multiple embedded field conditions
    let exact_match = Location::filter(
        Location::fields()
            .coords()
            .x()
            .eq(10)
            .and(Location::fields().coords().y().eq(20))
            .and(Location::fields().coords().z().eq(0)),
    )
    .collect::<Vec<_>>(&db)
    .await?;

    assert_eq!(exact_match.len(), 1);
    assert_eq!(exact_match[0].name, "Point A");
    Ok(())
}

/// Tests UPDATE operations filtered by embedded struct fields.
/// SQL-only: DynamoDB requires partition key in queries/updates.
/// Validates that updates can target rows based on embedded field values.
#[driver_test]
pub async fn update_with_embedded_field_filter(t: &mut Test) -> Result<()> {
    // Skip on non-SQL databases (DynamoDB requires partition key in queries/updates)
    if !t.capability().sql {
        return Ok(());
    }
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        version: i64,
        status: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
        meta: Metadata,
    }

    let db = t.setup_db(models!(Document, Metadata)).await;

    // Setup: Doc A (v1, draft), Doc B (v2, draft), Doc C (v1, published)
    for (title, version, status) in [
        ("Doc A", 1, "draft"),
        ("Doc B", 2, "draft"),
        ("Doc C", 1, "published"),
    ] {
        Document::create()
            .title(title)
            .meta(Metadata {
                version,
                status: status.to_string(),
            })
            .exec(&db)
            .await?;
    }

    // Update documents where status="draft" AND version=1 (should only match Doc A)
    // Tests that embedded field filters work in UPDATE statements
    Document::filter(
        Document::fields()
            .meta()
            .status()
            .eq("draft")
            .and(Document::fields().meta().version().eq(1)),
    )
    .update()
    .meta(Metadata {
        version: 2,
        status: "draft".to_string(),
    })
    .exec(&db)
    .await?;

    // Doc A should be updated (was v1 draft, now v2 draft)
    let doc_a = Document::filter(Document::fields().title().eq("Doc A"))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(doc_a[0].meta.version, 2);

    // Doc B should be unchanged (was v2 draft, still v2 draft)
    let doc_b = Document::filter(Document::fields().title().eq("Doc B"))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(doc_b[0].meta.version, 2);

    // Doc C should be unchanged (was v1 published, still v1 published - wrong status)
    let doc_c = Document::filter(Document::fields().title().eq("Doc C"))
        .collect::<Vec<_>>(&db)
        .await?;
    assert_eq!(doc_c[0].meta.version, 1);
    Ok(())
}

/// Tests partial updates of embedded struct fields using with_field() builders.
/// This validates that we can update individual fields within an embedded struct
/// without replacing the entire struct.
#[driver_test(id(ID))]
pub async fn partial_update_embedded_fields(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        address: Address,
    }

    let db = t.setup_db(models!(User, Address)).await;

    // Create a user with initial address
    let mut user = User::create()
        .name("Alice")
        .address(Address {
            street: "123 Main St".to_string(),
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        })
        .exec(&db)
        .await?;

    // Verify initial state
    assert_struct!(user.address, _ {
        street: "123 Main St",
        city: "Boston",
        zip: "02101",
        ..
    });

    // Partial update: only change city, leave street and zip unchanged
    user.update()
        .with_address(|a| {
            a.city("Seattle");
        })
        .exec(&db)
        .await?;

    // Verify only city was updated
    assert_struct!(user.address, _ {
        street: "123 Main St",
        city: "Seattle",
        zip: "02101",
        ..
    });

    // Verify the update persisted to database
    let found = User::get_by_id(&db, &user.id).await?;
    assert_struct!(found.address, _ {
        street: "123 Main St",
        city: "Seattle",
        zip: "02101",
        ..
    });

    // Multiple field update in one call
    user.update()
        .with_address(|a| {
            a.city("Portland").zip("97201");
        })
        .exec(&db)
        .await?;

    // Verify both fields were updated, street unchanged
    assert_struct!(user.address, _ {
        street: "123 Main St",
        city: "Portland",
        zip: "97201",
        ..
    });

    // Verify the update persisted
    let found = User::get_by_id(&db, &user.id).await?;
    assert_struct!(found.address, _ {
        street: "123 Main St",
        city: "Portland",
        zip: "97201",
        ..
    });

    // Multiple calls to with_address should accumulate
    user.update()
        .with_address(|a| {
            a.street("456 Oak Ave");
        })
        .with_address(|a| {
            a.zip("97202");
        })
        .exec(&db)
        .await?;

    // Verify all updates applied in memory
    assert_struct!(user.address, _ {
        street: "456 Oak Ave",
        city: "Portland",
        zip: "97202",
        ..
    });

    // Verify both accumulated assignments persisted to the database
    let found = User::get_by_id(&db, &user.id).await?;
    assert_struct!(found.address, _ {
        street: "456 Oak Ave",
        city: "Portland",
        zip: "97202",
        ..
    });
    Ok(())
}

/// Tests deeply nested embedded types (3+ levels) to verify schema building
/// handles arbitrary nesting depth correctly.
/// Validates:
/// - App schema: all embedded models registered
/// - DB schema: deeply nested fields flattened with proper prefixes
/// - Mapping: nested Field::Embedded structure with correct columns maps
/// - model_to_table: nested projection expressions
#[driver_test]
pub async fn deeply_nested_embedded_schema(test: &mut Test) {
    // 3 levels of nesting: Location -> City -> Address -> User
    #[derive(toasty::Embed)]
    struct Location {
        lat: i64,
        lon: i64,
    }

    #[derive(toasty::Embed)]
    struct City {
        name: String,
        location: Location,
    }

    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: City,
    }

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        address: Address,
    }

    let db = test.setup_db(models!(User, Address, City, Location)).await;
    let schema = db.schema();

    // All embedded models should exist in app schema
    assert_struct!(schema.app.models, #{
        Location::id(): _ {
            name.upper_camel_case(): "Location",
            kind: toasty::schema::app::ModelKind::Embedded,
            fields.len(): 2,
            ..
        },
        City::id(): _ {
            name.upper_camel_case(): "City",
            kind: toasty::schema::app::ModelKind::Embedded,
            fields: [
                _ { name.app_name: "name", .. },
                _ {
                    name.app_name: "location",
                    ty: FieldTy::Embedded(_ {
                        target: == Location::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        },
        Address::id(): _ {
            name.upper_camel_case(): "Address",
            kind: toasty::schema::app::ModelKind::Embedded,
            fields: [
                _ { name.app_name: "street", .. },
                _ {
                    name.app_name: "city",
                    ty: FieldTy::Embedded(_ {
                        target: == City::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        },
        User::id(): _ {
            name.upper_camel_case(): "User",
            kind: toasty::schema::app::ModelKind::Root(_),
            fields: [
                _ { name.app_name: "id", .. },
                _ {
                    name.app_name: "address",
                    ty: FieldTy::Embedded(_ {
                        target: == Address::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        },
    });

    // Database table should flatten all nested fields with proper prefixes
    // Expected columns:
    // - id
    // - address_street
    // - address_city_name
    // - address_city_location_lat
    // - address_city_location_lon
    assert_struct!(schema.db.tables, [
        _ {
            name: =~ r"users$",
            columns: [
                _ { name: "id", .. },
                _ { name: "address_street", .. },
                _ { name: "address_city_name", .. },
                _ { name: "address_city_location_lat", .. },
                _ { name: "address_city_location_lon", .. },
            ],
            ..
        }
    ]);

    let user = &schema.app.models[&User::id()];
    let user_table = schema.table_for(user);
    let user_mapping = &schema.mapping.models[&User::id()];

    // Mapping should have nested Field::Embedded structure
    // User.fields[1] (address) -> FieldEmbedded {
    //   fields[0] (street) -> FieldPrimitive { column: address_street }
    //   fields[1] (city) -> FieldEmbedded {
    //     fields[0] (name) -> FieldPrimitive { column: address_city_name }
    //     fields[1] (location) -> FieldEmbedded {
    //       fields[0] (lat) -> FieldPrimitive { column: address_city_location_lat }
    //       fields[1] (lon) -> FieldPrimitive { column: address_city_location_lon }
    //     }
    //   }
    // }

    assert_eq!(
        user_mapping.fields.len(),
        2,
        "User should have 2 fields: id and address"
    );

    // Check address field (index 1)
    let address_field = user_mapping.fields[1]
        .as_embedded()
        .expect("User.address should be Field::Embedded");

    assert_eq!(
        address_field.fields.len(),
        2,
        "Address should have 2 fields: street and city"
    );

    // Check address.street (index 0)
    let street_field = address_field.fields[0]
        .as_primitive()
        .expect("Address.street should be Field::Primitive");
    assert_eq!(
        street_field.column, user_table.columns[1].id,
        "street should map to address_street column"
    );

    // Check address.city (index 1)
    let city_field = address_field.fields[1]
        .as_embedded()
        .expect("Address.city should be Field::Embedded");

    assert_eq!(
        city_field.fields.len(),
        2,
        "City should have 2 fields: name and location"
    );

    // Check address.city.name (index 0)
    let city_name_field = city_field.fields[0]
        .as_primitive()
        .expect("City.name should be Field::Primitive");
    assert_eq!(
        city_name_field.column, user_table.columns[2].id,
        "city.name should map to address_city_name column"
    );

    // Check address.city.location (index 1)
    let location_field = city_field.fields[1]
        .as_embedded()
        .expect("City.location should be Field::Embedded");

    assert_eq!(
        location_field.fields.len(),
        2,
        "Location should have 2 fields: lat and lon"
    );

    // Check address.city.location.lat (index 0)
    let lat_field = location_field.fields[0]
        .as_primitive()
        .expect("Location.lat should be Field::Primitive");
    assert_eq!(
        lat_field.column, user_table.columns[3].id,
        "location.lat should map to address_city_location_lat column"
    );

    // Check address.city.location.lon (index 1)
    let lon_field = location_field.fields[1]
        .as_primitive()
        .expect("Location.lon should be Field::Primitive");
    assert_eq!(
        lon_field.column, user_table.columns[4].id,
        "location.lon should map to address_city_location_lon column"
    );

    // Check that the columns map is correctly populated at each level
    // Address level should contain all 4 columns (street, city_name, city_location_lat, city_location_lon)
    assert_eq!(
        address_field.columns.len(),
        4,
        "Address.columns should have 4 entries"
    );
    assert!(
        address_field
            .columns
            .contains_key(&user_table.columns[1].id),
        "Address.columns should contain address_street"
    );
    assert!(
        address_field
            .columns
            .contains_key(&user_table.columns[2].id),
        "Address.columns should contain address_city_name"
    );
    assert!(
        address_field
            .columns
            .contains_key(&user_table.columns[3].id),
        "Address.columns should contain address_city_location_lat"
    );
    assert!(
        address_field
            .columns
            .contains_key(&user_table.columns[4].id),
        "Address.columns should contain address_city_location_lon"
    );

    // City level should contain 3 columns (name, location_lat, location_lon)
    assert_eq!(
        city_field.columns.len(),
        3,
        "City.columns should have 3 entries"
    );
    assert!(
        city_field.columns.contains_key(&user_table.columns[2].id),
        "City.columns should contain address_city_name"
    );
    assert!(
        city_field.columns.contains_key(&user_table.columns[3].id),
        "City.columns should contain address_city_location_lat"
    );
    assert!(
        city_field.columns.contains_key(&user_table.columns[4].id),
        "City.columns should contain address_city_location_lon"
    );

    // Location level should contain 2 columns (lat, lon)
    assert_eq!(
        location_field.columns.len(),
        2,
        "Location.columns should have 2 entries"
    );
    assert!(
        location_field
            .columns
            .contains_key(&user_table.columns[3].id),
        "Location.columns should contain address_city_location_lat"
    );
    assert!(
        location_field
            .columns
            .contains_key(&user_table.columns[4].id),
        "Location.columns should contain address_city_location_lon"
    );

    // Verify model_to_table has correct nested projection expressions
    // Should have 5 expressions: id, address.street, address.city.name, address.city.location.lat, address.city.location.lon
    assert_eq!(
        user_mapping.model_to_table.len(),
        5,
        "model_to_table should have 5 expressions"
    );

    // Expression for address.street should be: project(ref(address_field), [0])
    assert_struct!(
        user_mapping.model_to_table[1],
        == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [0])
    );

    // Expression for address.city.name should be: project(ref(address_field), [1, 0])
    assert_struct!(
        user_mapping.model_to_table[2],
        == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1, 0])
    );

    // Expression for address.city.location.lat should be: project(ref(address_field), [1, 1, 0])
    assert_struct!(
        user_mapping.model_to_table[3],
        == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1, 1, 0])
    );

    // Expression for address.city.location.lon should be: project(ref(address_field), [1, 1, 1])
    assert_struct!(
        user_mapping.model_to_table[4],
        == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1, 1, 1])
    );
}

/// Tests CRUD operations with 2-level nested embedded structs.
/// Validates that creating, reading, updating (instance and query-based),
/// and deleting records with nested embedded structs works end-to-end.
#[driver_test(id(ID))]
pub async fn crud_nested_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Office {
        name: String,
        address: Address,
    }

    #[derive(Debug, toasty::Model)]
    struct Company {
        #[key]
        #[auto]
        id: ID,
        name: String,
        headquarters: Office,
    }

    let db = t.setup_db(models!(Company, Office, Address)).await;

    // Create: nested embedded structs are flattened into a single row
    let mut company = Company::create()
        .name("Acme")
        .headquarters(Office {
            name: "Main Office".to_string(),
            address: Address {
                street: "123 Main St".to_string(),
                city: "Springfield".to_string(),
            },
        })
        .exec(&db)
        .await?;

    assert_struct!(company.headquarters, _ {
        name: "Main Office",
        address: _ {
            street: "123 Main St",
            city: "Springfield",
            ..
        },
        ..
    });

    // Read: nested embedded struct is reconstructed from flattened columns
    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "Main Office",
        address: _ {
            street: "123 Main St",
            city: "Springfield",
            ..
        },
        ..
    });

    // Update (instance): replace the entire nested embedded struct
    company
        .update()
        .headquarters(Office {
            name: "West Coast HQ".to_string(),
            address: Address {
                street: "456 Oak Ave".to_string(),
                city: "Seattle".to_string(),
            },
        })
        .exec(&db)
        .await?;

    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "West Coast HQ",
        address: _ {
            street: "456 Oak Ave",
            city: "Seattle",
            ..
        },
        ..
    });

    // Update (query-based): replace nested struct via filter
    Company::filter_by_id(company.id)
        .update()
        .headquarters(Office {
            name: "East Coast HQ".to_string(),
            address: Address {
                street: "789 Pine Rd".to_string(),
                city: "Boston".to_string(),
            },
        })
        .exec(&db)
        .await?;

    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "East Coast HQ",
        address: _ {
            street: "789 Pine Rd",
            city: "Boston",
            ..
        },
        ..
    });

    // Delete: cleanup
    let id = company.id;
    company.delete(&db).await?;
    assert_err!(Company::get_by_id(&db, &id).await);
    Ok(())
}

/// Tests partial updates of deeply nested embedded fields using chained closures.
/// Validates that `with_outer(|o| o.with_inner(|i| i.field(v)))` updates only
/// the targeted leaf field, leaving all other fields unchanged in the database.
#[driver_test(id(ID))]
pub async fn partial_update_nested_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Office {
        name: String,
        address: Address,
    }

    #[derive(Debug, toasty::Model)]
    struct Company {
        #[key]
        #[auto]
        id: ID,
        name: String,
        headquarters: Office,
    }

    let db = t.setup_db(models!(Company, Office, Address)).await;

    let mut company = Company::create()
        .name("Acme")
        .headquarters(Office {
            name: "Main Office".to_string(),
            address: Address {
                street: "123 Main St".to_string(),
                city: "Boston".to_string(),
            },
        })
        .exec(&db)
        .await?;

    // Nested partial update: change only the city inside headquarters.address.
    // street and headquarters.name must remain unchanged.
    company
        .update()
        .with_headquarters(|h| {
            h.with_address(|a| {
                a.city("Seattle");
            });
        })
        .exec(&db)
        .await?;

    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "Main Office",
        address: _ {
            street: "123 Main St",
            city: "Seattle",
            ..
        },
        ..
    });

    // Partial update at the outer level: change only headquarters.name.
    // address fields must remain unchanged.
    company
        .update()
        .with_headquarters(|h| {
            h.name("West Coast HQ");
        })
        .exec(&db)
        .await?;

    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "West Coast HQ",
        address: _ {
            street: "123 Main St",
            city: "Seattle",
            ..
        },
        ..
    });

    // Combined update: change headquarters.name and headquarters.address.city
    // in a single with_headquarters call. street must remain unchanged.
    company
        .update()
        .with_headquarters(|h| {
            h.name("East Coast HQ").with_address(|a| {
                a.city("Boston");
            });
        })
        .exec(&db)
        .await?;

    let found = Company::get_by_id(&db, &company.id).await?;
    assert_struct!(found.headquarters, _ {
        name: "East Coast HQ",
        address: _ {
            street: "123 Main St",
            city: "Boston",
            ..
        },
        ..
    });
    Ok(())
}

/// Tests partial updates of embedded fields using the query/filter-based path.
/// `User::filter_by_id(id).update().with_address(...)` follows a different code path
/// than the instance-based `user.update().with_address(...)`, so both need coverage.
#[driver_test(id(ID))]
pub async fn query_based_partial_update_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        address: Address,
    }

    let db = t.setup_db(models!(User, Address)).await;

    let user = User::create()
        .name("Alice")
        .address(Address {
            street: "123 Main St".to_string(),
            city: "Boston".to_string(),
            zip: "02101".to_string(),
        })
        .exec(&db)
        .await?;

    // Single field: filter-based partial update targeting only city.
    // street and zip must remain unchanged.
    User::filter_by_id(user.id)
        .update()
        .with_address(|a| {
            a.city("Seattle");
        })
        .exec(&db)
        .await?;

    let found = User::get_by_id(&db, &user.id).await?;
    assert_struct!(found.address, _ {
        street: "123 Main St",
        city: "Seattle",
        zip: "02101",
        ..
    });

    // Multiple fields: update city and zip together, leave street unchanged.
    User::filter_by_id(user.id)
        .update()
        .with_address(|a| {
            a.city("Portland").zip("97201");
        })
        .exec(&db)
        .await?;

    let found = User::get_by_id(&db, &user.id).await?;
    assert_struct!(found.address, _ {
        street: "123 Main St",
        city: "Portland",
        zip: "97201",
        ..
    });
    Ok(())
}
