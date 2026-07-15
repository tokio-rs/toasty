use crate::prelude::*;

#[driver_test(id(ID), requires(upsert_primary_key))]
pub async fn upsert_by_primary_key_creates_then_updates(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id)
        .value("created")
        .exec(&mut db)
        .await?;
    assert_eq!(created.id, id);
    assert_eq!(created.value, "created");

    let updated = Item::upsert_by_id(id)
        .value("updated")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.id, id);
    assert_eq!(updated.value, "updated");
    assert_eq!(Item::get_by_id(&mut db, id).await?.value, "updated");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_branch_assignments))]
pub async fn upsert_branch_overrides(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id)
        .on_create(|create| create.value("created"))
        .on_update(|update| update.value("updated"))
        .exec(&mut db)
        .await?;
    assert_eq!(created.value, "created");

    let updated = Item::upsert_by_id(id)
        .on_create(|create| create.value("created again"))
        .on_update(|update| update.value("updated"))
        .exec(&mut db)
        .await?;
    assert_eq!(updated.value, "updated");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_targeted_ignore))]
pub async fn upsert_or_ignore_returns_option(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id)
        .value("first")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert_eq!(created.unwrap().value, "first");

    let ignored = Item::upsert_by_id(id)
        .value("second")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert!(ignored.is_none());
    assert_eq!(Item::get_by_id(&mut db, id).await?.value, "first");
    Ok(())
}

#[driver_test(id(ID), requires(and(upsert_primary_key, upsert_targeted_ignore)))]
pub async fn upsert_or_ignore_suppresses_only_the_selected_conflict(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[unique]
        email: String,
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;
    let first_seed = User::create()
        .email("first-seed@example.com")
        .name("seed")
        .exec(&mut db)
        .await?;
    let first_id = first_seed.id;
    first_seed.delete().exec(&mut db).await?;

    let second_seed = User::create()
        .email("second-seed@example.com")
        .name("seed")
        .exec(&mut db)
        .await?;
    let second_id = second_seed.id;
    second_seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(first_id)
        .email("alice@example.com")
        .name("Alice")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert_eq!(created.unwrap().email, "alice@example.com");

    let ignored = User::upsert_by_id(first_id)
        .email("other@example.com")
        .name("Other")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert!(ignored.is_none());
    assert_eq!(
        User::get_by_id(&mut db, first_id).await?.email,
        "alice@example.com"
    );

    let other_conflict = User::upsert_by_id(second_id)
        .email("alice@example.com")
        .name("Duplicate")
        .or_ignore()
        .exec(&mut db)
        .await;
    assert!(other_conflict.is_err());
    Ok(())
}

#[driver_test(id(ID), requires(upsert_unique))]
pub async fn upsert_by_unique_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[unique]
        email: String,
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;
    let created = User::upsert_by_email("alice@example.com")
        .name("Alice")
        .exec(&mut db)
        .await?;
    let updated = User::upsert_by_email("alice@example.com")
        .name("Alicia")
        .exec(&mut db)
        .await?;

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.email, "alice@example.com");
    assert_eq!(updated.name, "Alicia");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_unique))]
pub async fn upsert_by_composite_unique_constraint(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[unique(tenant, slug)]
    struct Entry {
        #[key]
        #[auto]
        id: ID,
        tenant: String,
        slug: String,
        value: String,
    }

    let mut db = test.setup_db(models!(Entry)).await;
    let created = Entry::upsert_by_tenant_and_slug("acme", "home")
        .value("one")
        .exec(&mut db)
        .await?;
    let updated = Entry::upsert_by_tenant_and_slug("acme", "home")
        .value("two")
        .exec(&mut db)
        .await?;

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.value, "two");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_primary_key))]
pub async fn upsert_applies_model_defaults(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
        #[default("created".to_string())]
        created_only: String,
        #[update("always".to_string())]
        updated_always: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id).value("one").exec(&mut db).await?;
    assert_eq!(created.created_only, "created");
    assert_eq!(created.updated_always, "always");

    let updated = Item::upsert_by_id(id).value("two").exec(&mut db).await?;
    assert_eq!(updated.created_only, "created");
    assert_eq!(updated.updated_always, "always");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_branch_assignments))]
pub async fn upsert_update_can_reference_incoming_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let id = seed.id;

    let updated = Item::upsert_by_id(id)
        .value("incoming")
        .on_update(|update| {
            let incoming = update.incoming();
            update.value(incoming.value())
        })
        .exec(&mut db)
        .await?;
    assert_eq!(updated.value, "incoming");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_branch_assignments))]
pub async fn upsert_update_can_reference_stored_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = toasty::create!(Item { value: "stored" })
        .exec(&mut db)
        .await?;

    let updated = Item::upsert_by_id(seed.id)
        .value("incoming")
        .on_update(|update| update.value(toasty::stmt::set(Item::fields().value())))
        .exec(&mut db)
        .await?;
    assert_eq!(updated.value, "stored");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_primary_key))]
pub async fn upsert_shared_assignment_operators(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        count: i64,
        tags: Vec<String>,
        note: Option<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create()
        .count(0)
        .tags(Vec::<String>::new())
        .exec(&mut db)
        .await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id)
        .count(toasty::stmt::increment())
        .tags(toasty::stmt::push("a"))
        .note(Some("present".to_string()))
        .exec(&mut db)
        .await?;
    assert_eq!(created.count, 1);
    assert_eq!(created.tags, ["a"]);
    assert_eq!(created.note.as_deref(), Some("present"));

    let incremented = Item::upsert_by_id(id)
        .count(toasty::stmt::increment())
        .tags(toasty::stmt::push("b"))
        .note(None::<String>)
        .exec(&mut db)
        .await?;
    assert_eq!(incremented.count, 2);
    assert_eq!(incremented.tags, ["a", "b"]);
    assert_eq!(incremented.note, None);

    let decremented = Item::upsert_by_id(id)
        .count(toasty::stmt::subtract(1_i64))
        .tags(toasty::stmt::extend(Vec::<String>::new()))
        .exec(&mut db)
        .await?;
    assert_eq!(decremented.count, 1);
    assert_eq!(decremented.tags, ["a", "b"]);
    Ok(())
}

#[driver_test(id(ID), requires(upsert_primary_key))]
pub async fn upsert_without_update_assignments_is_invalid(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().exec(&mut db).await?;
    let error = Item::upsert_by_id(seed.id).exec(&mut db).await.unwrap_err();
    assert!(error.is_invalid_statement(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(and(upsert_primary_key, not(upsert_branch_assignments)))
)]
pub async fn unsupported_upsert_branches_are_reported_before_dispatch(
    test: &mut Test,
) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let error = Item::upsert_by_id(seed.id)
        .value("shared")
        .on_update(|update| update.value("updated"))
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(id(ID), requires(and(upsert_primary_key, not(upsert_unique))))]
pub async fn unsupported_unique_upsert_is_reported_before_dispatch(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[unique]
        email: String,
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;
    let error = User::upsert_by_email("alice@example.com")
        .name("Alice")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(id(ID), requires(not(upsert_primary_key)))]
pub async fn unsupported_upsert_is_reported_before_dispatch(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        value: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = Item::create().value("seed").exec(&mut db).await?;
    let error = Item::upsert_by_id(seed.id)
        .value("updated")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}
