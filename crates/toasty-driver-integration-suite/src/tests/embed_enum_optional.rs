//! `Option<EmbeddedEnum>` model fields.
//!
//! A nullable embedded enum reuses its discriminant column as the head: `NULL`
//! = `None`, a variant value = `Some(variant)`. The data-enum decode `Match`
//! gets a `null` else branch so a `NULL` (unmatched) discriminant decodes to
//! `None`; a unit-only enum's discriminant column reads back as `None` directly.
//!
//! Variant-only predicates (`.is_email()`) on the optional path are not yet
//! exposed — that needs the same `Field` path-machinery work deferred for
//! nested-field access on `Option<struct>`. Whole-value `.eq(Some(Variant))`
//! covers variant matching here.

use crate::helpers::column;
use crate::prelude::*;

use toasty_core::stmt::{Expr, Value};

/// The DB schema: nullable discriminant columns (named after the field) plus
/// nullable variant-field columns for the data enum.
#[driver_test(scenario(crate::scenarios::account_optional_contact_status))]
pub async fn option_enum_db_schema(test: &mut Test) {
    let db = setup(test).await;
    let schema = db.schema();

    assert_struct!(schema.db.tables, [
        {
            name: =~ r"accounts$",
            columns: [
                { name: "id" },
                { name: "contact", nullable: true },
                { name: "contact_address", nullable: true },
                { name: "contact_number", nullable: true },
                { name: "status", nullable: true },
            ],
        },
    ]);
}

/// Round-trip `Some(variant)` and `None` for both a data enum and a unit enum.
#[driver_test(scenario(crate::scenarios::account_optional_contact_status))]
pub async fn option_enum_crud(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Account {
        id: "full",
        contact: Some(Contact::Email {
            address: "a@x.com".to_string(),
        }),
        status: Some(Status::Active),
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Account {
        id: "empty",
        contact: None,
        status: None,
    })
    .exec(&mut db)
    .await?;

    let full = Account::get_by_id(&mut db, "full").await?;
    assert_struct!(full, _ {
        contact: Some(== Contact::Email { address: "a@x.com".to_string() }),
        status: Some(== Status::Active),
        ..
    });

    let empty = Account::get_by_id(&mut db, "empty").await?;
    assert_none!(empty.contact);
    assert_none!(empty.status);

    Ok(())
}

/// `.is_none()` / `.is_some()` on an `Option<enum>` filter the discriminant
/// column's null-ness.
#[driver_test(
    requires(scan),
    scenario(crate::scenarios::account_optional_contact_status)
)]
pub async fn option_enum_filter_presence(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Account {
        id: "full",
        contact: Some(Contact::Phone {
            number: "555".to_string(),
        }),
        status: Some(Status::Inactive),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Account {
        id: "empty",
        contact: None,
        status: None,
    })
    .exec(&mut db)
    .await?;

    // Data enum.
    let present = Account::filter(Account::fields().contact().is_some())
        .exec(&mut db)
        .await?;
    assert_struct!(present, [_ { id: "full", .. }]);

    let absent = Account::filter(Account::fields().contact().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(absent, [_ { id: "empty", .. }]);

    // Unit enum.
    let status_absent = Account::filter(Account::fields().status().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(status_absent, [_ { id: "empty", .. }]);

    Ok(())
}

/// Whole-value `.eq(Some(variant))` matches the equal `Some` row and never
/// `None`, for both a data variant and a unit variant.
#[driver_test(
    requires(scan),
    scenario(crate::scenarios::account_optional_contact_status)
)]
pub async fn option_enum_filter_eq(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Account {
        id: "alice",
        contact: Some(Contact::Email {
            address: "alice@x.com".to_string(),
        }),
        status: Some(Status::Active),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Account {
        id: "bob",
        contact: Some(Contact::Phone {
            number: "555".to_string(),
        }),
        status: Some(Status::Inactive),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Account {
        id: "empty",
        contact: None,
        status: None,
    })
    .exec(&mut db)
    .await?;

    let email = Account::filter(Account::fields().contact().eq(Some(Contact::Email {
        address: "alice@x.com".to_string(),
    })))
    .exec(&mut db)
    .await?;
    assert_struct!(email, [_ { id: "alice", .. }]);

    let active = Account::filter(Account::fields().status().eq(Some(Status::Active)))
        .exec(&mut db)
        .await?;
    assert_struct!(active, [_ { id: "alice", .. }]);

    Ok(())
}

/// Updating the whole value in both directions for a data and a unit enum.
#[driver_test(scenario(crate::scenarios::account_optional_contact_status))]
pub async fn option_enum_update(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(Account {
        id: "a",
        contact: Some(Contact::Email {
            address: "a@x.com".to_string(),
        }),
        status: None,
    })
    .exec(&mut db)
    .await?;

    // Some -> None (data enum).
    Account::filter_by_id("a")
        .update()
        .contact(None)
        .exec(&mut db)
        .await?;
    assert_none!(Account::get_by_id(&mut db, "a").await?.contact);

    // None -> Some (unit enum).
    Account::filter_by_id("a")
        .update()
        .status(Some(Status::Active))
        .exec(&mut db)
        .await?;
    assert_struct!(
        Account::get_by_id(&mut db, "a").await?.status,
        Some(== Status::Active)
    );

    Ok(())
}

/// Driver-op coverage for create. A `Some(variant)` writes the variant
/// discriminant to the (shared) discriminant column and the variant's fields to
/// their columns (other variants' columns `NULL`); a `None` writes `NULL` to the
/// discriminant *and* every variant column.
#[driver_test(scenario(crate::scenarios::account_optional_contact_status))]
pub async fn option_enum_create_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let contact = column(&db, "accounts", "contact").index;
    let address = column(&db, "accounts", "contact_address").index;
    let number = column(&db, "accounts", "contact_number").index;
    let status = column(&db, "accounts", "status").index;
    test.log().clear();

    // Some: discriminant is the variant value (Email = 1, Active = 1); only the
    // active variant's column is set.
    toasty::create!(Account {
        id: "a",
        contact: Some(Contact::Email {
            address: "a@x.com".to_string(),
        }),
        status: Some(Status::Active),
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&contact], Value::from(1i64));
    assert_eq!(row[&address], Value::from("a@x.com"));
    assert_eq!(row[&number], Value::Null);
    assert_eq!(row[&status], Value::from(1i64));

    // None: discriminant and every variant column are NULL.
    toasty::create!(Account {
        id: "b",
        contact: None,
        status: None,
    })
    .exec(&mut db)
    .await?;
    let row = pop_insert(test);
    assert_eq!(row[&contact], Value::Null);
    assert_eq!(row[&address], Value::Null);
    assert_eq!(row[&number], Value::Null);
    assert_eq!(row[&status], Value::Null);

    Ok(())
}

/// Driver-op coverage for filters. `.is_none()` / `.is_some()` emit a predicate
/// on the single discriminant column (`IS NULL` / `NOT (.. IS NULL)`), never a
/// check distributed across the variant columns.
#[driver_test(
    requires(scan),
    scenario(crate::scenarios::account_optional_contact_status)
)]
pub async fn option_enum_filter_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let contact = column(&db, "accounts", "contact").index;
    test.log().clear();

    // `.is_none()` → `contact IS NULL`.
    let _ = Account::filter(Account::fields().contact().is_none())
        .exec(&mut db)
        .await?;
    assert_struct!(pop_filter(test), Expr::IsNull({
        expr.as_expr_column_unwrap().column: == contact,
    }));

    // `.is_some()` → `NOT (contact IS NULL)`.
    let _ = Account::filter(Account::fields().contact().is_some())
        .exec(&mut db)
        .await?;
    let Expr::Not(not) = pop_filter(test) else {
        panic!("expected NOT");
    };
    assert_struct!(*not.expr, Expr::IsNull({
        expr.as_expr_column_unwrap().column: == contact,
    }));

    Ok(())
}

/// Driver-op coverage for updates. Setting `Some(variant)` assigns the variant
/// discriminant + that variant's column (others `NULL`); setting `None` assigns
/// `NULL` to the discriminant and every variant column.
#[driver_test(scenario(crate::scenarios::account_optional_contact_status))]
pub async fn option_enum_update_ops(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let contact = column(&db, "accounts", "contact").index;
    let address = column(&db, "accounts", "contact_address").index;
    let number = column(&db, "accounts", "contact_number").index;

    toasty::create!(Account {
        id: "a",
        contact: None,
        status: None,
    })
    .exec(&mut db)
    .await?;

    // None -> Some(Phone): discriminant = 2, number set, address NULL.
    test.log().clear();
    Account::filter_by_id("a")
        .update()
        .contact(Some(Contact::Phone {
            number: "555".to_string(),
        }))
        .exec(&mut db)
        .await?;
    let set = pop_update(test);
    assert_eq!(set[&contact], Value::from(2i64));
    assert_eq!(set[&number], Value::from("555"));
    assert_eq!(set[&address], Value::Null);

    // Some -> None: discriminant and every variant column NULL.
    test.log().clear();
    Account::filter_by_id("a")
        .update()
        .contact(None)
        .exec(&mut db)
        .await?;
    let set = pop_update(test);
    assert_eq!(set[&contact], Value::Null);
    assert_eq!(set[&address], Value::Null);
    assert_eq!(set[&number], Value::Null);

    Ok(())
}
