#![allow(clippy::disallowed_names)]

use crate::prelude::*;

#[driver_test(id(ID))]
pub async fn missing_registration_belongs_to(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        bar_id: ID,
        #[belongs_to(key = bar_id, references = id)]
        bar: toasty::BelongsTo<Bar>,
    }

    #[derive(Debug, toasty::Model)]
    struct Bar {
        #[key]
        #[auto]
        id: ID,
    }

    let error = t.try_setup_db(models!(Foo)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Foo::bar"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_has_one(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[has_one]
        bar: toasty::HasOne<Bar>,
    }

    #[derive(Debug, toasty::Model)]
    struct Bar {
        #[key]
        #[auto]
        id: ID,

        foo_id: ID,
        #[belongs_to(key = foo_id, references = id)]
        foo: toasty::BelongsTo<Foo>,
    }

    let error = t.try_setup_db(models!(Foo)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Foo::bar"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_has_many(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        #[has_many]
        bars: toasty::HasMany<Bar>,
    }

    #[derive(Debug, toasty::Model)]
    struct Bar {
        #[key]
        #[auto]
        id: ID,

        #[index]
        foo_id: ID,
        #[belongs_to(key = foo_id, references = id)]
        foo: toasty::BelongsTo<Foo>,
    }

    let error = t.try_setup_db(models!(Foo)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Foo::bar"));

    Ok(())
}

#[driver_test(id(ID))]
pub async fn missing_registration_embedded(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,

        bar: Bar,
    }

    #[derive(Debug, toasty::Embed)]
    struct Bar {
        x: i32,
    }

    let error = t.try_setup_db(models!(Foo)).await.unwrap_err();
    assert!(error.is_invalid_schema());
    assert!(format!("{error}").contains("Foo::bar"));

    Ok(())
}
