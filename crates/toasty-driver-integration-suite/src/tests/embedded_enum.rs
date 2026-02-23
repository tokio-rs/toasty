use toasty::schema::{
    app::FieldTy,
    mapping::{self, FieldPrimitive},
};

use crate::prelude::*;

/// Tests that embedded enums are registered in the app schema but don't create
/// their own database tables (they're inlined into parent models as a single column).
#[driver_test]
pub async fn basic_embedded_enum(test: &mut Test) {
    #[derive(toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    let db = test.setup_db(models!(Status)).await;
    let schema = db.schema();

    // Embedded enums exist in app schema as Model::EmbeddedEnum
    assert_struct!(schema.app.models, #{
        Status::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            name.upper_camel_case(): "Status",
            variants: [
                _ { name.upper_camel_case(): "Pending", discriminant: 1, .. },
                _ { name.upper_camel_case(): "Active", discriminant: 2, .. },
                _ { name.upper_camel_case(): "Done", discriminant: 3, .. },
            ],
            ..
        }),
    });

    // Embedded enums don't create database tables (stored as a column in parent)
    assert!(schema.db.tables.is_empty());
}

/// Tests the complete schema generation and mapping for an embedded enum field:
/// - App schema: enum field with correct type reference
/// - DB schema: enum field stored as a single INTEGER column
/// - Mapping: enum field maps directly to a primitive column (discriminant IS the value)
#[driver_test]
pub async fn root_model_with_embedded_enum_field(test: &mut Test) {
    #[derive(toasty::Embed)]
    enum Status {
        #[column(variant = 1)]
        Pending,
        #[column(variant = 2)]
        Active,
        #[column(variant = 3)]
        Done,
    }

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        status: Status,
    }

    let db = test.setup_db(models!(User, Status)).await;
    let schema = db.schema();

    // Both embedded enum and root model exist in app schema
    assert_struct!(schema.app.models, #{
        Status::id(): toasty::schema::app::Model::EmbeddedEnum(_ {
            name.upper_camel_case(): "Status",
            variants.len(): 3,
            ..
        }),
        User::id(): toasty::schema::app::Model::Root(_ {
            name.upper_camel_case(): "User",
            fields: [
                _ { name.app_name: "id", .. },
                _ {
                    name.app_name: "status",
                    ty: FieldTy::Embedded(_ {
                        target: == Status::id(),
                        ..
                    }),
                    ..
                }
            ],
            ..
        }),
    });

    // Database table has a single INTEGER column for the enum discriminant
    assert_struct!(schema.db.tables, [
        _ {
            name: =~ r"users$",
            columns: [
                _ { name: "id", .. },
                _ { name: "status", .. },
            ],
            ..
        }
    ]);

    let user = &schema.app.models[&User::id()];
    let user_table = schema.table_for(user);
    let user_mapping = &schema.mapping.models[&User::id()];

    // Enum field maps directly to a primitive column — discriminant IS the stored value,
    // no record wrapping needed (unlike embedded structs which use Field::Embedded).
    assert_struct!(user_mapping, _ {
        columns.len(): 2,
        fields: [
            mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[0].id,
                lowering: 0,
                ..
            }),
            mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[1].id,
                lowering: 1,
                ..
            }),
        ],
        ..
    });
}
