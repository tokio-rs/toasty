//! Tests for whole-value unique constraints on `Vec<scalar>` fields.

use crate::prelude::*;

#[driver_test(requires(unique_list_index))]
pub async fn unique_vec_uses_ordered_complete_value(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        tags: Vec<String>,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item {
        tags: ["rust", "toasty"],
    })
    .exec(&mut db)
    .await?;

    assert_err!(
        toasty::create!(Item {
            tags: ["rust", "toasty"],
        })
        .exec(&mut db)
        .await
    );

    toasty::create!(Item {
        tags: ["toasty", "rust"],
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Item {
        tags: ["rust", "rust"],
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Item {
        tags: Vec::<String>::new(),
    })
    .exec(&mut db)
    .await?;

    assert_err!(
        toasty::create!(Item {
            tags: Vec::<String>::new(),
        })
        .exec(&mut db)
        .await
    );

    Ok(())
}

#[driver_test(requires(and(unique_list_index, upsert_unique)))]
pub async fn unique_vec_generated_operations(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        tags: Vec<String>,
        name: String,
    }

    let mut db = t.setup_db(models!(Item)).await;
    let item = toasty::create!(Item {
        tags: ["one", "two"],
        name: "original",
    })
    .exec(&mut db)
    .await?;

    let found = Item::get_by_tags(&mut db, ["one", "two"]).await?;
    assert_eq!(found.id, item.id);

    let filtered = Item::filter_by_tags(["one", "two"]).exec(&mut db).await?;
    assert_eq!(filtered.len(), 1);

    Item::update_by_tags(["one", "two"])
        .tags(["three"])
        .exec(&mut db)
        .await?;

    let updated = Item::upsert_by_tags(["three"])
        .name("updated")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.id, item.id);
    assert_eq!(updated.name, "updated");

    Item::delete_by_tags(&mut db, ["three"]).await?;
    assert_none!(
        Item::filter_by_tags(["three"])
            .first()
            .exec(&mut db)
            .await?
    );

    Ok(())
}

#[driver_test(requires(unique_list_index))]
pub async fn unique_vec_newtype(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Tags(Vec<String>);

    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        tags: Tags,
    }

    let mut db = t.setup_db(models!(Item)).await;

    toasty::create!(Item {
        tags: Tags(vec!["rust".into(), "toasty".into()]),
    })
    .exec(&mut db)
    .await?;

    assert_err!(
        toasty::create!(Item {
            tags: Tags(vec!["rust".into(), "toasty".into()]),
        })
        .exec(&mut db)
        .await
    );

    let found = Item::get_by_tags(&mut db, Tags(vec!["rust".into(), "toasty".into()])).await?;
    assert_eq!(found.tags.0, ["rust", "toasty"]);

    Ok(())
}

#[driver_test(requires(and(vec_scalar, not(unique_list_index))))]
pub async fn unique_vec_unsupported_backend(t: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        tags: Vec<String>,
    }

    let err = assert_err!(t.try_setup_db(models!(Item)).await);
    assert!(err.is_unsupported_feature());

    let message = err.to_string();
    assert!(
        message.contains("#[unique]")
            && message.contains("Vec<T>")
            && message.contains("complete collection values"),
        "unexpected schema-build error: {message}"
    );
}

#[driver_test(requires(vec_scalar))]
pub async fn non_unique_vec_index_is_rejected(t: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        tags: Vec<String>,
    }

    let err = assert_err!(t.try_setup_db(models!(Item)).await);
    assert!(err.is_unsupported_feature());

    let message = err.to_string();
    assert!(
        message.contains("#[index]") && message.contains("Vec<T>"),
        "unexpected schema-build error: {message}"
    );
}
