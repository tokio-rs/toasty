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
