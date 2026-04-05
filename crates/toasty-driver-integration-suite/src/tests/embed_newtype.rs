use toasty::schema::mapping::{self, FieldPrimitive, FieldStruct};

use crate::prelude::*;

/// Tests that a newtype embedded struct (`struct Email(String)`) is registered
/// in the app schema with `app: None` on its inner field.
#[driver_test]
pub async fn basic_newtype_embed(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    let db = test.setup_db(models!(Email)).await;
    let schema = db.schema();

    assert_struct!(schema.app.models, #{
        Email::id(): toasty::schema::app::Model::EmbeddedStruct({
            name.upper_camel_case(): "Email",
            fields: [
                { name.app: None },
            ],
        }),
    });

    assert!(schema.db.tables.is_empty());
}

/// Tests that a newtype field produces a single column whose name matches the
/// parent field — `email: Email` where `struct Email(String)` produces column
/// `email`, not `email_0`.
#[driver_test]
pub async fn newtype_column_name(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        #[allow(dead_code)]
        email: Email,
    }

    let db = test.setup_db(models!(User, Email)).await;
    let schema = db.schema();

    assert_struct!(schema.db.tables, [
        {
            name: =~ r"users$",
            columns: [
                { name: "id" },
                { name: "email" },
            ],
        },
    ]);

    let user = &schema.app.models[&User::id()];
    let user_mapping = &schema.mapping.models[&User::id()];
    let user_table = schema.table_for(user);

    assert_struct!(user_mapping, {
        columns.len(): 2,
        fields: [
            mapping::Field::Primitive(FieldPrimitive {
                column: == user_table.columns[0].id,
                ..
            }),
            mapping::Field::Struct(FieldStruct {
                fields: [
                    mapping::Field::Primitive(FieldPrimitive {
                        column: == user_table.columns[1].id,
                        ..
                    }),
                ],
                ..
            }),
        ],
    });
}

/// Tests full CRUD with a newtype embedded field.
#[driver_test(id(ID))]
pub async fn crud_newtype_embed(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        email: Email,
    }

    let mut db = t.setup_db(models!(User, Email)).await;

    // Create
    let user = toasty::create!(User {
        name: "Alice",
        email: Email("alice@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Alice");
    assert_eq!(user.email.0, "alice@example.com");

    // Read back
    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.email.0, "alice@example.com");

    Ok(())
}

/// Tests filtering by a newtype embedded field using `==` comparison.
#[driver_test(id(ID))]
pub async fn filter_newtype_embed_eq(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        email: Email,
    }

    let mut db = t.setup_db(models!(User, Email)).await;

    toasty::create!(User {
        name: "Alice",
        email: Email("alice@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(User {
        name: "Bob",
        email: Email("bob@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    let users = User::filter(
        User::fields()
            .email()
            .eq(Email("alice@example.com".into())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}
