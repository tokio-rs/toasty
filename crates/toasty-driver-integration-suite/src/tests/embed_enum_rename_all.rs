use crate::prelude::*;

use toasty::Db;
use toasty_core::schema::db;

/// The (driver-independent) storage type of a database column.
fn column_storage_ty(db: &Db, table: &str, column: &str) -> db::Type {
    let schema = db.schema();
    let table = schema
        .db
        .tables
        .iter()
        .find(|t| t.name == table || t.name.ends_with(table))
        .unwrap_or_else(|| panic!("table '{table}' not in schema"));
    table
        .columns
        .iter()
        .find(|c| c.name == column)
        .unwrap_or_else(|| panic!("column '{column}' not in table"))
        .storage_ty
        .clone()
}

/// The (type name, variant labels) of a native enum column, in declaration
/// order. Panics if the column is not a native enum type.
fn native_enum(db: &Db, table: &str, column: &str) -> (String, Vec<String>) {
    match column_storage_ty(db, table, column) {
        db::Type::Enum(e) => (
            e.name.expect("a native enum type has a name"),
            e.variants.into_iter().map(|v| v.name).collect(),
        ),
        other => panic!("expected a native enum column, got {other:?}"),
    }
}

/// `#[column(rename_all = ...)]` derives each variant's default label; an
/// explicit per-variant `#[column(variant = ...)]` still wins.
#[driver_test]
pub async fn rename_all_derives_labels(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase")]
    enum Pascal {
        Customer,
        PreferredSupplier,
    }

    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "SCREAMING_SNAKE_CASE")]
    enum Screaming {
        Customer,
        PreferredSupplier,
    }

    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase")]
    enum Overridden {
        #[column(variant = "vip")]
        PreferredSupplier,
        Customer,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Contact {
        #[key]
        #[auto]
        id: uuid::Uuid,
        pascal: Pascal,
        screaming: Screaming,
        overridden: Overridden,
    }

    let db = t.setup_db(models!(Contact)).await;

    assert_eq!(
        native_enum(&db, "contacts", "pascal").1,
        ["Customer", "PreferredSupplier"]
    );
    assert_eq!(
        native_enum(&db, "contacts", "screaming").1,
        ["CUSTOMER", "PREFERRED_SUPPLIER"]
    );
    assert_eq!(
        native_enum(&db, "contacts", "overridden").1,
        ["vip", "Customer"]
    );
}

/// The renamed labels flow into every string storage mapping: native enum
/// (default, explicit `type = enum`, and named `type = enum("...")`) and the
/// plain `type = text` column. `rename_all` never affects the enum type name.
#[driver_test]
pub async fn rename_all_across_storage_mappings(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase")]
    enum NativeDefault {
        Customer,
        Supplier,
    }

    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase", type = enum)]
    enum NativeExplicit {
        Customer,
        Supplier,
    }

    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase", type = enum("party_kind"))]
    enum NativeNamed {
        Customer,
        Supplier,
    }

    #[derive(Debug, toasty::Embed)]
    #[column(rename_all = "PascalCase", type = text)]
    enum TextMapped {
        Customer,
        Supplier,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Contact {
        #[key]
        #[auto]
        id: uuid::Uuid,
        native_default: NativeDefault,
        native_explicit: NativeExplicit,
        native_named: NativeNamed,
        text_mapped: TextMapped,
    }

    let db = t.setup_db(models!(Contact)).await;
    let renamed = ["Customer".to_string(), "Supplier".to_string()];

    // Default native enum: type name derived from the ident in snake_case.
    let (name, labels) = native_enum(&db, "contacts", "native_default");
    assert!(name.ends_with("native_default"), "unexpected name: {name}");
    assert_eq!(labels, renamed);

    // Explicit `type = enum`: identical native representation.
    assert_eq!(native_enum(&db, "contacts", "native_explicit").1, renamed);

    // Named `type = enum("party_kind")`: custom type name, renamed labels.
    let (name, labels) = native_enum(&db, "contacts", "native_named");
    assert!(name.ends_with("party_kind"), "unexpected name: {name}");
    assert_eq!(labels, renamed);

    // Plain text mapping: a TEXT column, not a native enum type.
    assert_eq!(
        column_storage_ty(&db, "contacts", "text_mapped"),
        db::Type::Text
    );
}

/// End-to-end: a renamed enum round-trips through the database (create + read).
#[driver_test]
pub async fn rename_all_round_trip(t: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[column(rename_all = "PascalCase")]
    enum PartyKind {
        Customer,
        Supplier,
    }

    #[derive(Debug, toasty::Model)]
    struct Contact {
        #[key]
        #[auto]
        id: uuid::Uuid,
        kind: PartyKind,
    }

    let mut db = t.setup_db(models!(Contact)).await;

    let contact = toasty::create!(Contact {
        kind: PartyKind::Supplier,
    })
    .exec(&mut db)
    .await?;
    let found = Contact::get_by_id(&mut db, &contact.id).await?;
    assert_eq!(found.kind, PartyKind::Supplier);

    Ok(())
}
