use crate::prelude::*;

/// A newtype tuple struct with #[derive(Embed)] should be transparent to the
/// database — it maps to a single column using the inner type's storage.
#[driver_test(id(ID))]
pub async fn newtype_crud(t: &mut Test) -> Result<()> {
    #[derive(Debug, Clone, PartialEq, toasty::Embed)]
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
    let mut user = toasty::create!(User {
        name: "Alice",
        email: Email("alice@example.com".to_string()),
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.email.0, "alice@example.com");

    // Read
    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.email.0, "alice@example.com");

    // Update (instance)
    user.update()
        .email(Email("alice@new.com".to_string()))
        .exec(&mut db)
        .await?;

    let found = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(found.email.0, "alice@new.com");

    // Delete
    let id = user.id;
    user.delete().exec(&mut db).await?;
    assert_err!(User::get_by_id(&mut db, &id).await);

    Ok(())
}

/// Newtype column should use the parent field name directly, not
/// a prefixed subfield name.
#[driver_test]
pub async fn newtype_schema_single_column(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        id: String,
        email: Email,
    }

    let db = test.setup_db(models!(User, Email)).await;
    let schema = db.schema();

    // The newtype field maps to a single column named "email" (not "email_field0")
    assert_struct!(schema.db.tables, [
        {
            name: =~ r"users$",
            columns: [
                { name: "id" },
                { name: "email" },
            ],
        },
    ]);
}

/// Newtype fields can be used in filter expressions.
#[driver_test(id(ID))]
pub async fn newtype_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, Clone, PartialEq, toasty::Embed)]
    struct Email(String);

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        email: Email,
    }

    let mut db = t.setup_db(models!(User, Email)).await;

    toasty::create!(User {
        email: Email("a@example.com".to_string()),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(User {
        email: Email("b@example.com".to_string()),
    })
    .exec(&mut db)
    .await?;

    let found = User::filter(
        User::fields()
            .email()
            .eq(Email("a@example.com".to_string())),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].email.0, "a@example.com");

    Ok(())
}
