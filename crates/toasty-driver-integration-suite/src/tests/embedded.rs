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
        id: toasty::stmt::Id<Self>,
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
            }),
            mapping::Field::Embedded(FieldEmbedded {
                fields: [
                    mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[1].id,
                        lowering: 1,
                    }),
                    mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[2].id,
                        lowering: 2,
                    })
                ],
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
        id: toasty::stmt::Id<Self>,
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
