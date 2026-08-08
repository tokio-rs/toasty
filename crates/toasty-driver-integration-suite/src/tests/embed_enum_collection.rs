//! `Vec<unit-enum>` collections. The element is a scalar discriminant, so the
//! column is a native scalar array where the backend has one — `int8[]` for
//! integer discriminants, `ink[]` for a native enum — never a document.

use crate::helpers::column;
use crate::prelude::*;

use toasty_core::{schema::db, stmt};

#[derive(Clone, Copy, Debug, PartialEq, toasty::Embed)]
enum Color {
    #[column(variant = 1)]
    Red,
    #[column(variant = 2)]
    Green,
    #[column(variant = 3)]
    Blue,
}

#[derive(Debug, toasty::Model)]
struct Palette {
    #[key]
    #[auto]
    id: uuid::Uuid,
    colors: Vec<Color>,
}

/// A `Vec<unit-enum>` round-trips through INSERT and a fresh fetch.
#[driver_test(requires(document_collections))]
pub async fn vec_enum_create_get(t: &mut Test) -> Result<(), BoxError> {
    let mut db = t.setup_db(models!(Palette)).await;

    let colors = [Color::Red, Color::Blue];
    let palette = toasty::create!(Palette { colors }).exec(&mut db).await?;

    let reloaded = Palette::get_by_id(&mut db, &palette.id).await?;
    assert_eq!(reloaded.colors, colors);

    Ok(())
}

/// The scalar-collection operators unlocked by the emitted `Scalar` impl:
/// `contains(variant)` matches on discriminant membership and `len()` filters
/// on cardinality.
#[driver_test(requires(document_collections))]
pub async fn vec_enum_contains_and_len(t: &mut Test) -> Result<(), BoxError> {
    let mut db = t.setup_db(models!(Palette)).await;

    toasty::create!(Palette::[
        { colors: [Color::Red, Color::Green] },
        { colors: [Color::Blue] },
        { colors: [Color::Red, Color::Green, Color::Blue] },
    ])
    .exec(&mut db)
    .await?;

    let reds = Palette::filter(Palette::fields().colors().contains(Color::Red))
        .exec(&mut db)
        .await?;
    assert_eq!(reds.len(), 2);

    let singletons = Palette::filter(Palette::fields().colors().len().eq(1))
        .exec(&mut db)
        .await?;
    assert_eq!(singletons.len(), 1);

    let triples = Palette::filter(Palette::fields().colors().len().eq(3))
        .exec(&mut db)
        .await?;
    assert_eq!(triples.len(), 1);

    Ok(())
}

/// Enum-level storage applies to collection elements, while a field-level
/// type overrides that default. Both paths bridge and round-trip each element.
#[driver_test(requires(document_collections))]
pub async fn vec_enum_uses_discriminant_storage(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Copy, Debug, PartialEq, toasty::Embed)]
    #[column(type = u16)]
    enum SmallColor {
        #[column(variant = 1)]
        Red,
        #[column(variant = 2)]
        Green,
    }

    #[derive(Debug, toasty::Model)]
    struct SmallPalette {
        #[key]
        #[auto]
        id: uuid::Uuid,
        colors: Vec<SmallColor>,
        #[column(type = u8)]
        compact_colors: Vec<SmallColor>,
    }

    let mut db = t.setup_db(models!(SmallPalette)).await;

    assert_eq!(
        column_storage_ty(&db, "small_palettes", "colors"),
        db::Type::list(db::Type::UnsignedInteger(2))
    );
    assert_eq!(
        column_storage_ty(&db, "small_palettes", "compact_colors"),
        db::Type::list(db::Type::UnsignedInteger(1))
    );

    let colors_column = db
        .schema()
        .db
        .column(column(&db, "small_palettes", "colors"));
    assert_eq!(
        colors_column.ty,
        stmt::Type::List(Box::new(stmt::Type::U16))
    );
    let compact_column = db
        .schema()
        .db
        .column(column(&db, "small_palettes", "compact_colors"));
    assert_eq!(
        compact_column.ty,
        stmt::Type::List(Box::new(stmt::Type::U8))
    );

    let colors = [SmallColor::Red, SmallColor::Green];
    let palette = toasty::create!(SmallPalette {
        colors,
        compact_colors: colors,
    })
    .exec(&mut db)
    .await?;

    let reloaded = SmallPalette::get_by_id(&mut db, &palette.id).await?;
    assert_eq!(reloaded.colors, colors);
    assert_eq!(reloaded.compact_colors, colors);

    Ok(())
}

/// Transparent field wrappers preserve enum-level storage, and a field-level
/// override still wins after passing through the wrapper.
#[driver_test]
pub async fn enum_storage_propagates_through_wrappers(t: &mut Test) -> Result<(), BoxError> {
    #[derive(Clone, Copy, Debug, PartialEq, toasty::Embed)]
    #[column(type = u16)]
    enum Status {
        #[column(variant = 1)]
        Draft,
        #[column(variant = 2)]
        Published,
    }

    #[derive(Debug, toasty::Model)]
    struct WrappedStatus {
        #[key]
        #[auto]
        id: uuid::Uuid,
        optional: Option<Status>,
        deferred: toasty::Deferred<Status>,
        boxed: Box<Status>,
        arced: std::sync::Arc<Status>,
        #[column(type = u8)]
        rced: std::rc::Rc<Status>,
    }

    let mut db = t.setup_db(models!(WrappedStatus)).await;

    for name in ["optional", "deferred", "boxed", "arced"] {
        assert_eq!(
            column_storage_ty(&db, "wrapped_statuses", name),
            db::Type::UnsignedInteger(2)
        );
    }
    assert_eq!(
        column_storage_ty(&db, "wrapped_statuses", "rced"),
        db::Type::UnsignedInteger(1)
    );

    let wrapped = toasty::create!(WrappedStatus {
        optional: Some(Status::Draft),
        deferred: Status::Published,
        boxed: Status::Draft,
        arced: Status::Published,
        rced: Status::Draft,
    })
    .exec(&mut db)
    .await?;

    let reloaded = WrappedStatus::filter_by_id(wrapped.id)
        .include(WrappedStatus::fields().deferred())
        .get(&mut db)
        .await?;
    assert_eq!(reloaded.optional, Some(Status::Draft));
    assert_eq!(*reloaded.deferred.get(), Status::Published);
    assert_eq!(*reloaded.boxed, Status::Draft);
    assert_eq!(*reloaded.arced, Status::Published);
    assert_eq!(*reloaded.rced, Status::Draft);

    Ok(())
}

/// Enum storage also follows an enum nested through a flattened embed.
#[driver_test]
pub async fn enum_storage_propagates_through_nested_embed(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    #[column(type = u8)]
    enum Status {
        #[column(variant = 1)]
        Active,
        #[column(variant = 2)]
        Archived,
    }

    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        status: Status,
    }

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        metadata: Metadata,
    }

    let db = t.setup_db(models!(Item)).await;
    assert_eq!(
        column_storage_ty(&db, "items", "metadata_status"),
        db::Type::UnsignedInteger(1)
    );
}

#[derive(Clone, Copy, Debug, PartialEq, toasty::Embed)]
enum Ink {
    Cyan,
    Magenta,
    Yellow,
}

#[derive(Debug, toasty::Model)]
struct Printer {
    #[key]
    #[auto]
    id: uuid::Uuid,
    inks: Vec<Ink>,
}

/// A `Vec<native-enum>` stores as a native enum array (`ink[]`), not `text[]`.
#[driver_test(requires(and(native_enum, native_array)))]
pub async fn native_enum_vec_stores_as_enum_array(t: &mut Test) -> Result<(), BoxError> {
    let db = t.setup_db(models!(Printer)).await;

    let storage_ty = column_storage_ty(&db, "printers", "inks");
    let db::Type::List(elem) = &storage_ty else {
        panic!("expected List(Enum), got {storage_ty:?}")
    };
    let db::Type::Enum(type_enum) = &**elem else {
        panic!("expected List(Enum), got {storage_ty:?}")
    };

    // Type name is test-prefixed, so assert on the variant labels.
    let variants: Vec<&str> = type_enum.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(variants, ["cyan", "magenta", "yellow"]);

    Ok(())
}

/// A `Vec<native-enum>` round-trips through INSERT and a fresh fetch,
/// exercising the enum-array bind and decode wire paths.
#[driver_test(requires(and(native_enum, vec_scalar)))]
pub async fn native_enum_vec_create_get(t: &mut Test) -> Result<(), BoxError> {
    let mut db = t.setup_db(models!(Printer)).await;

    let inks = [Ink::Cyan, Ink::Yellow];
    let printer = toasty::create!(Printer { inks }).exec(&mut db).await?;

    let reloaded = Printer::get_by_id(&mut db, &printer.id).await?;
    assert_eq!(reloaded.inks, inks);

    Ok(())
}
