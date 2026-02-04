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
            Some(mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[0].id,
                lowering: 0,
            })),
            Some(mapping::Field::Embedded(FieldEmbedded {
                fields: [
                    Some(mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[1].id,
                        lowering: 1,
                    })),
                    Some(mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[2].id,
                        lowering: 2,
                    }))
                ],
            })),
        ],
        model_to_table.fields: [
            == stmt::Expr::cast(stmt::Expr::ref_self_field(user.fields[0].id), stmt::Type::String),
            == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [0]),
            == stmt::Expr::project(stmt::Expr::ref_self_field(user.fields[1].id), [1])
        ],
        ..
    });

    // Verify table -> model mapping (lifting)
    // Should construct model values from table columns

    // Get the expression for the address field (index 1)
    let address_expr = user_mapping.table_to_model.lower_expr_reference(0, 1);

    // Should be a record that constructs Address from columns
    // record(column[1], column[2]) builds Address { street, city }
    assert!(
        address_expr.is_record(),
        "table_to_model expression for address field should be a record"
    );

    // Verify the Address record has 2 fields (street and city)
    let address_record = address_expr.as_record().unwrap();
    assert_eq!(
        address_record.fields.len(),
        2,
        "Address record should have 2 fields (street, city)"
    );
}
