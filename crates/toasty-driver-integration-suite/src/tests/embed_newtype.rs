use toasty::schema::mapping::{self, FieldPrimitive, FieldStruct};

#[allow(unused_imports)]
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

/// Tests create, read-back, eq filter, update, delete-by-filter, and batch
/// create — all with the same `Email(String)` newtype model.
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

    // Create + read back
    let mut user = toasty::create!(User {
        name: "Alice",
        email: Email("alice@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Alice");
    assert_eq!(user.email.0, "alice@example.com");

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.email.0, "alice@example.com");

    // Update
    user.update()
        .email(Email("new@example.com".into()))
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.email.0, "new@example.com");

    // Eq filter
    toasty::create!(User {
        name: "Bob",
        email: Email("bob@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    let users = User::filter(User::fields().email().eq(Email("bob@example.com".into())))
        .exec(&mut db)
        .await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");

    // Delete by filter
    User::filter(User::fields().email().eq(Email("bob@example.com".into())))
        .delete()
        .exec(&mut db)
        .await?;

    let all = User::all().exec(&mut db).await?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Alice");

    // Batch create
    User::create_many()
        .item(
            User::create()
                .name("Carol")
                .email(Email("carol@example.com".into())),
        )
        .item(
            User::create()
                .name("Dave")
                .email(Email("dave@example.com".into())),
        )
        .exec(&mut db)
        .await?;

    let all = User::all().exec(&mut db).await?;
    assert_eq!(all.len(), 3);

    Ok(())
}

// TODO: ne(), gt(), lt(), ge(), le() are not yet generated for newtype field
// structs. Add comparison operator tests once codegen is extended.

/// Tests `#[unique]` on a newtype field generates `get_by_*` and enforces
/// uniqueness.
#[ignore] // indexed field should map to a primitive column
#[driver_test(id(ID))]
pub async fn newtype_unique_constraint(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        #[unique]
        email: Email,
    }

    let mut db = t.setup_db(models!(User, Email)).await;

    toasty::create!(User {
        name: "Alice",
        email: Email("alice@example.com".into()),
    })
    .exec(&mut db)
    .await?;

    // Duplicate email should fail
    assert_err!(
        toasty::create!(User {
            name: "Bob",
            email: Email("alice@example.com".into()),
        })
        .exec(&mut db)
        .await
    );

    // get_by_email should work
    let found = User::get_by_email(&mut db, Email("alice@example.com".into())).await?;
    assert_eq!(found.name, "Alice");

    Ok(())
}

/// Tests `#[index]` on a newtype field generates `filter_by_*`.
#[ignore] // indexed field should map to a primitive column
#[driver_test(id(ID))]
pub async fn newtype_index(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        #[index]
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

    let users = User::filter_by_email(Email("alice@example.com".into()))
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}

/// Tests a newtype wrapping a numeric type with CRUD and eq filter.
#[driver_test(id(ID))]
pub async fn newtype_numeric(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Score(i64);

    #[derive(Debug, toasty::Model)]
    struct Player {
        #[key]
        #[auto]
        id: ID,
        name: String,
        score: Score,
    }

    let mut db = t.setup_db(models!(Player, Score)).await;

    toasty::create!(Player {
        name: "Alice",
        score: Score(100)
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Player {
        name: "Bob",
        score: Score(200)
    })
    .exec(&mut db)
    .await?;

    let found = Player::filter(Player::fields().score().eq(Score(200)))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Bob");

    Ok(())
}

/// Tests using a newtype as the primary key field.
#[ignore] // indexed field should map to a primitive column
#[driver_test]
pub async fn newtype_as_primary_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct UserId(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: UserId,
        name: String,
    }

    let mut db = t.setup_db(models!(User, UserId)).await;

    let user = toasty::create!(User {
        id: UserId("user-1".into()),
        name: "Alice",
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.id.0, "user-1");

    let found = User::get_by_id(&mut db, &UserId("user-1".into())).await?;
    assert_eq!(found.name, "Alice");

    Ok(())
}

/// Tests newtype nested inside an embedded struct: create, read-back, and
/// filter by the nested newtype field.
#[driver_test(id(ID))]
pub async fn nested_newtype(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct ZipCode(String);

    #[derive(Debug, toasty::Embed)]
    struct Address {
        city: String,
        zip: ZipCode,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        name: String,
        address: Address,
    }

    let mut db = t.setup_db(models!(User, Address, ZipCode)).await;

    // Create + read back
    let user = toasty::create!(User {
        name: "Alice",
        address: Address {
            city: "Seattle".into(),
            zip: ZipCode("98101".into()),
        },
    })
    .exec(&mut db)
    .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.address.city, "Seattle");
    assert_eq!(found.address.zip.0, "98101");

    // Filter by nested newtype
    toasty::create!(User {
        name: "Bob",
        address: Address {
            city: "Portland".into(),
            zip: ZipCode("97201".into()),
        },
    })
    .exec(&mut db)
    .await?;

    let users = User::filter(User::fields().address().zip().eq(ZipCode("98101".into())))
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");

    Ok(())
}
