use toasty::schema::{
    app::FieldTy,
    mapping::{self, FieldEmbedded, FieldPrimitive},
};
use toasty_core::stmt;

use crate::prelude::*;

#[driver_test]
pub async fn basic_embedded_struct(test: &mut Test) {
    #[derive(toasty::Embed)]
    struct Address {
        street: String,
        city: String,
    }

    let db = test.setup_db(models!(Address)).await;
    let schema = db.schema();

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

    assert!(schema.db.tables.is_empty());
}

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

    // Verify both models in app-level schema
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

    // Verify mapping - embedded fields should have projection expressions
    let user = &schema.app.models[&User::id()];
    let user_table = schema.table_for(user);
    let user_mapping = &schema.mapping.models[&User::id()];

    // Verify model -> table mapping (lowering)
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

#[driver_test(id(ID))]
pub async fn create_and_query_embedded(t: &mut Test) {
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

    // Create a user with an embedded address
    let mut user = User::create()
        .name("Alice")
        .address(Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
        })
        .exec(&db)
        .await
        .unwrap();

    // Query the user back
    let found = User::get_by_id(&db, &user.id).await.unwrap();

    assert_eq!(found.name, "Alice");
    assert_eq!(found.address.street, "123 Main St");
    assert_eq!(found.address.city, "Springfield");

    // Update using instance method
    user.update()
        .address(Address {
            street: "456 Oak Ave".to_string(),
            city: "Shelbyville".to_string(),
        })
        .exec(&db)
        .await
        .unwrap();

    // Reload and verify update
    let found = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(found.name, "Alice");
    assert_eq!(found.address.street, "456 Oak Ave");
    assert_eq!(found.address.city, "Shelbyville");

    // Update using filter_by_id pattern
    User::filter_by_id(user.id)
        .update()
        .name("Bob")
        .address(Address {
            street: "789 Pine Rd".to_string(),
            city: "Capital City".to_string(),
        })
        .exec(&db)
        .await
        .unwrap();

    // Reload and verify second update
    let found = User::get_by_id(&db, &user.id).await.unwrap();
    assert_eq!(found.name, "Bob");
    assert_eq!(found.address.street, "789 Pine Rd");
    assert_eq!(found.address.city, "Capital City");

    // Delete the user
    let id = user.id;
    user.delete(&db).await.unwrap();

    // Verify deletion
    assert_err!(User::get_by_id(&db, &id).await);
}

#[driver_test]
pub async fn embedded_struct_fields_codegen(test: &mut Test) {
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
        id: toasty::stmt::Id<Self>,
        name: String,
        address: Address,
    }

    let _db = test.setup_db(models!(User, Address)).await;

    // Test that User::fields().address() returns AddressFields
    // and we can directly chain to access nested fields
    let _city_path = User::fields().address().city();
    let _street_path = User::fields().address().street();
    let _zip_path = User::fields().address().zip();

    // Test that AddressFields works correctly when returned from User::fields()
    let address_fields = User::fields().address();
    let _city_path_2 = address_fields.city();
    let _street_path_2 = address_fields.street();
    let _zip_path_2 = address_fields.zip();

    // Test that Address::fields() also works directly
    let _address_city = Address::fields().city();
    let _address_street = Address::fields().street();
    let _address_zip = Address::fields().zip();

    // Verify the paths have the correct type for use in queries
    // (This is a compile-time check that the types are correct)

    // Test that the path can be used in a filter expression
    let _query = User::all().filter(User::fields().address().city().eq("Seattle"));
}

#[driver_test]
pub async fn query_embedded_struct_fields(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        street: String,
        city: String,
        zip: String,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = country, local = id)]
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
            .await
            .unwrap();
    }

    // Verify all users were created by querying each partition
    // (DynamoDB with composite keys requires partition key in queries)
    let mut all_users = Vec::new();
    for country in ["USA", "CAN"] {
        let mut users = User::filter(User::fields().country().eq(country))
            .collect::<Vec<_>>(&db)
            .await
            .unwrap();
        all_users.append(&mut users);
    }
    assert_eq!(all_users.len(), 7);

    // Verify basic partition key filtering works
    let usa_users = User::filter(User::fields().country().eq("USA"))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(usa_users.len(), 4);

    // Query by partition key (country) and embedded field (city)
    let seattle_users = User::filter(
        User::fields()
            .country()
            .eq("USA")
            .and(User::fields().address().city().eq("Seattle")),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(seattle_users.len(), 2);
    let mut names: Vec<_> = seattle_users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Alice", "Bob"]);

    // Verify the addresses are correct
    for user in &seattle_users {
        assert_eq!(user.address.city, "Seattle");
        assert_eq!(user.country, "USA");
    }

    // Query by partition key and different embedded field (zip)
    let portland_users = User::filter(
        User::fields()
            .country()
            .eq("USA")
            .and(User::fields().address().city().eq("Portland")),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(portland_users.len(), 2);
    let mut names: Vec<_> = portland_users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Charlie", "Diana"]);

    // Query Canadian users in Vancouver
    let vancouver_users = User::filter(
        User::fields()
            .country()
            .eq("CAN")
            .and(User::fields().address().city().eq("Vancouver")),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(vancouver_users.len(), 2);
    let mut names: Vec<_> = vancouver_users.iter().map(|u| u.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Eve", "Frank"]);

    // Verify we can query by zip code as well
    let user_98101 = User::filter(
        User::fields()
            .country()
            .eq("USA")
            .and(User::fields().address().zip().eq("98101")),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(user_98101.len(), 1);
    assert_eq!(user_98101[0].name, "Alice");
    assert_eq!(user_98101[0].address.street, "123 Main St");
}

#[driver_test(requires(sql))]
pub async fn query_embedded_fields_comparison_ops(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Stats {
        score: i64,
        rank: i64,
    }

    #[derive(Debug, toasty::Model)]
    struct Player {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        stats: Stats,
    }

    let db = t.setup_db(models!(Player, Stats)).await;

    // Create players with different scores
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
            .await
            .unwrap();
    }

    // Test greater than
    let high_scorers = Player::filter(Player::fields().stats().score().gt(80))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(high_scorers.len(), 2); // Alice and Bob

    // Test less than or equal
    let low_scorers = Player::filter(Player::fields().stats().score().le(55))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(low_scorers.len(), 2); // Diana and Eve

    // Test not equal
    let not_charlie = Player::filter(Player::fields().stats().score().ne(70))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(not_charlie.len(), 4);

    // Test greater than or equal
    let mid_to_high = Player::filter(Player::fields().stats().score().ge(70))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(mid_to_high.len(), 3); // Alice, Bob, Charlie
}

#[driver_test(requires(sql))]
pub async fn query_embedded_multiple_fields(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Coordinates {
        x: i64,
        y: i64,
        z: i64,
    }

    #[derive(Debug, toasty::Model)]
    struct Location {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        coords: Coordinates,
    }

    let db = t.setup_db(models!(Location, Coordinates)).await;

    // Create locations
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
            .await
            .unwrap();
    }

    // Query by multiple embedded fields: x=10 AND y=20
    let matching = Location::filter(
        Location::fields()
            .coords()
            .x()
            .eq(10)
            .and(Location::fields().coords().y().eq(20)),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(matching.len(), 2); // Point A and Point C
    let mut names: Vec<_> = matching.iter().map(|l| l.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Point A", "Point C"]);

    // Query by three embedded fields: x=10 AND y=20 AND z=0
    let exact_match = Location::filter(
        Location::fields()
            .coords()
            .x()
            .eq(10)
            .and(Location::fields().coords().y().eq(20))
            .and(Location::fields().coords().z().eq(0)),
    )
    .collect::<Vec<_>>(&db)
    .await
    .unwrap();

    assert_eq!(exact_match.len(), 1);
    assert_eq!(exact_match[0].name, "Point A");
}

#[driver_test(requires(sql))]
pub async fn update_with_embedded_field_filter(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        version: i64,
        status: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Document {
        #[key]
        #[auto]
        id: uuid::Uuid,
        title: String,
        meta: Metadata,
    }

    let db = t.setup_db(models!(Document, Metadata)).await;

    // Create documents
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
            .await
            .unwrap();
    }

    // Update all draft documents with version 1
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
    .await
    .unwrap();

    // Verify the update
    let updated = Document::filter(Document::fields().title().eq("Doc A"))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();

    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].meta.version, 2);
    assert_eq!(updated[0].meta.status, "draft");

    // Other documents should be unchanged
    let unchanged = Document::filter(Document::fields().title().eq("Doc B"))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(unchanged[0].meta.version, 2); // Already version 2

    let published = Document::filter(Document::fields().title().eq("Doc C"))
        .collect::<Vec<_>>(&db)
        .await
        .unwrap();
    assert_eq!(published[0].meta.version, 1); // Still version 1 (status was "published")
}
