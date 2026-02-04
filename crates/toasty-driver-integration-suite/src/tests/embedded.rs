use toasty::schema::app::FieldTy;
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
    let user_mapping = &schema.mapping.models[&User::id()];

    println!("{:#?}", user_mapping.model_to_table);

    assert_struct!(&schema.mapping.models[&User::id()], _ {
        columns.len(): 3,
        model_to_table.fields: [
            &stmt::Expr::cast(stmt::Expr::ref_self_field(user.fields[0].id), stmt::Type::String),
            == &stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [0]),
            == &stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1])
        ],
        ..
    });
}
