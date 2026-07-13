//! Tests for `Vec<enum>` where the element is a unit (data-less) enum. Such an
//! enum is a scalar discriminant, so the collection is stored as a document
//! array and offers the scalar-collection operators (`contains`, `len`, …).
//! Gated on `document_collections`, like the struct-embed collections in
//! `type_document`.

use crate::prelude::*;

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

/// A `Vec<unit-enum>` round-trips through INSERT and a fresh fetch: the
/// discriminants are stored as a document array and reloaded back to variants.
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
