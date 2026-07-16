//! `Vec<unit-enum>` collections. The element is a scalar discriminant, so the
//! column is a native scalar array where the backend has one — `int8[]` for
//! integer discriminants, `ink[]` for a native enum — never a document.

use crate::prelude::*;

use toasty_core::schema::db;

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
