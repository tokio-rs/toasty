use crate::prelude::*;

#[driver_test(
    id(ID),
    requires(upsert_primary_key),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_by_primary_key_creates_then_updates(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(id).name("created").exec(&mut db).await?;
    assert_eq!(created.id, id);
    assert_eq!(created.name, "created");

    let updated = User::upsert_by_id(id).name("updated").exec(&mut db).await?;
    assert_eq!(updated.id, id);
    assert_eq!(updated.name, "updated");
    assert_eq!(User::get_by_id(&mut db, id).await?.name, "updated");
    Ok(())
}

/// Upsert updates advance the OCC version, so a stale instance cannot overwrite
/// the updated row.
#[driver_test(
    requires(upsert_primary_key),
    scenario(crate::scenarios::versioned_item)
)]
pub async fn upsert_update_increments_version(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(Item { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let mut stale = Item::upsert_by_id(id).name("first").exec(&mut db).await?;
    assert_struct!(stale, _ { name: "first", version: 1, .. });

    let updated = Item::upsert_by_id(id).name("second").exec(&mut db).await?;
    assert_struct!(updated, _ { name: "second", version: 2, .. });

    let result: Result<()> = stale.update().name("stale").exec(&mut db).await;
    assert!(result.is_err(), "expected stale update to fail");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_branch_assignments),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_branch_overrides(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(id)
        .name("shared")
        .on_create(|create| create.name("created"))
        .on_update(|update| update.name("updated"))
        .exec(&mut db)
        .await?;
    assert_eq!(created.name, "created");

    let updated = User::upsert_by_id(id)
        .name("shared again")
        .on_create(|create| create.name("created again"))
        .on_update(|update| update.name("updated"))
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "updated");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_branch_assignments),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_branch_overrides_are_order_independent(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(id)
        .on_create(|create| create.name("created"))
        .on_update(|update| update.name("updated"))
        .name("shared")
        .exec(&mut db)
        .await?;
    assert_eq!(created.name, "created");

    let updated = User::upsert_by_id(id)
        .on_create(|create| create.name("created again"))
        .on_update(|update| update.name("updated"))
        .name("shared again")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "updated");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_branch_assignments),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_single_branch_override_keeps_shared_other_branch(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;
    let create_seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let create_id = create_seed.id;
    create_seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(create_id)
        .on_create(|create| create.name("created"))
        .name("shared")
        .exec(&mut db)
        .await?;
    assert_eq!(created.name, "created");

    let updated = User::upsert_by_id(create_id)
        .on_create(|create| create.name("created again"))
        .name("shared")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "shared");

    let update_seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let update_id = update_seed.id;
    update_seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(update_id)
        .on_update(|update| update.name("updated"))
        .name("shared")
        .exec(&mut db)
        .await?;
    assert_eq!(created.name, "shared");

    let updated = User::upsert_by_id(update_id)
        .on_update(|update| update.name("updated"))
        .name("shared again")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "updated");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_targeted_ignore),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_or_ignore_returns_option(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = User::upsert_by_id(id)
        .name("first")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert_eq!(created.unwrap().name, "first");

    let ignored = User::upsert_by_id(id)
        .name("second")
        .or_ignore()
        .exec(&mut db)
        .await?;
    assert!(ignored.is_none());
    assert_eq!(User::get_by_id(&mut db, id).await?.name, "first");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(and(upsert_primary_key, upsert_targeted_ignore)),
    scenario(crate::scenarios::user_unique_email_with_name)
)]
pub async fn upsert_or_ignore_suppresses_only_the_selected_conflict(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let first_seed = toasty::create!(User {
        email: "first-seed@example.com",
        name: "seed",
    })
    .exec(&mut db)
    .await?;
    let first_id = first_seed.id;
    first_seed.delete().exec(&mut db).await?;

    let second_seed = toasty::create!(User {
        email: "second-seed@example.com",
        name: "seed",
    })
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

#[driver_test(
    id(ID),
    requires(upsert_unique),
    scenario(crate::scenarios::user_unique_email_with_name)
)]
pub async fn upsert_by_unique_field(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
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

#[driver_test(
    id(ID),
    requires(upsert_unique),
    scenario(crate::scenarios::upsert_models)
)]
pub async fn upsert_by_composite_unique_constraint(test: &mut Test) -> Result<()> {
    let mut db = setup_entry(test).await;
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

#[driver_test(
    id(ID),
    requires(upsert_primary_key),
    scenario(crate::scenarios::upsert_models)
)]
pub async fn upsert_applies_model_defaults(test: &mut Test) -> Result<()> {
    let mut db = setup_defaulted_item(test).await;
    let seed = toasty::create!(DefaultedItem { value: "seed" })
        .exec(&mut db)
        .await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = DefaultedItem::upsert_by_id(id)
        .value("one")
        .exec(&mut db)
        .await?;
    assert_eq!(created.created_only, "created");
    assert_eq!(created.updated_always, "always");

    let updated = DefaultedItem::upsert_by_id(id)
        .value("two")
        .exec(&mut db)
        .await?;
    assert_eq!(updated.created_only, "created");
    assert_eq!(updated.updated_always, "always");
    Ok(())
}

#[driver_test(id(ID), requires(upsert_primary_key))]
pub async fn upsert_explicit_assignments_override_model_defaults(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: ID,

        #[default("create default".to_string())]
        default_only: String,

        #[update("update default".to_string())]
        update_only: String,

        #[default("create branch default".to_string())]
        #[update("update branch default".to_string())]
        branch_defaults: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let seed = toasty::create!(Item {}).exec(&mut db).await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = Item::upsert_by_id(id)
        .default_only("created explicitly")
        .update_only("created explicitly")
        .branch_defaults("created explicitly")
        .exec(&mut db)
        .await?;
    assert_struct!(created, _ {
        default_only: "created explicitly",
        update_only: "created explicitly",
        branch_defaults: "created explicitly",
        ..
    });

    let updated = Item::upsert_by_id(id)
        .default_only("updated explicitly")
        .update_only("updated explicitly")
        .branch_defaults("updated explicitly")
        .exec(&mut db)
        .await?;
    assert_struct!(updated, _ {
        default_only: "updated explicitly",
        update_only: "updated explicitly",
        branch_defaults: "updated explicitly",
        ..
    });

    assert_none!(Item::upsert_by_id(id).or_ignore().exec(&mut db).await?);
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_primary_key),
    scenario(crate::scenarios::upsert_models)
)]
pub async fn upsert_create_assignment_initializes_only_new_record(test: &mut Test) -> Result<()> {
    let mut db = setup_defaulted_item(test).await;
    let seed = toasty::create!(DefaultedItem { value: "seed" })
        .exec(&mut db)
        .await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = DefaultedItem::upsert_by_id(id)
        .value("one")
        .on_create(|create| create.created_only("initial"))
        .exec(&mut db)
        .await?;
    assert_eq!(created.created_only, "initial");

    let updated = DefaultedItem::upsert_by_id(id)
        .value("two")
        .on_create(|create| create.created_only("replacement"))
        .exec(&mut db)
        .await?;
    assert_eq!(updated.created_only, "initial");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_branch_assignments),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_update_can_reference_incoming_value(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let id = seed.id;

    let updated = User::upsert_by_id(id)
        .name("shared")
        .on_create(|create| create.name("created"))
        .on_update(|update| {
            let incoming = update.incoming();
            update.name(incoming.name())
        })
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "created");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_branch_assignments),
    scenario(crate::scenarios::two_models)
)]
pub async fn upsert_update_can_reference_stored_value(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "stored" })
        .exec(&mut db)
        .await?;

    let updated = User::upsert_by_id(seed.id)
        .name("incoming")
        .on_update(|update| update.name(toasty::stmt::set(User::fields().name())))
        .exec(&mut db)
        .await?;
    assert_eq!(updated.name, "stored");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_primary_key),
    scenario(crate::scenarios::upsert_models)
)]
pub async fn upsert_shared_assignment_operators(test: &mut Test) -> Result<()> {
    let mut db = setup_assigned_item(test).await;
    let seed = toasty::create!(AssignedItem {
        count: 0,
        tags: Vec::<String>::new(),
    })
    .exec(&mut db)
    .await?;
    let id = seed.id;
    seed.delete().exec(&mut db).await?;

    let created = AssignedItem::upsert_by_id(id)
        .count(toasty::stmt::increment())
        .tags(toasty::stmt::push("a"))
        .note(Some("present".to_string()))
        .exec(&mut db)
        .await?;
    assert_eq!(created.count, 1);
    assert_eq!(created.tags, ["a"]);
    assert_eq!(created.note.as_deref(), Some("present"));

    let incremented = AssignedItem::upsert_by_id(id)
        .count(toasty::stmt::increment())
        .tags(toasty::stmt::push("b"))
        .note(None::<String>)
        .exec(&mut db)
        .await?;
    assert_eq!(incremented.count, 2);
    assert_eq!(incremented.tags, ["a", "b"]);
    assert_eq!(incremented.note, None);

    let decremented = AssignedItem::upsert_by_id(id)
        .count(toasty::stmt::subtract(1_i64))
        .tags(toasty::stmt::extend(Vec::<String>::new()))
        .exec(&mut db)
        .await?;
    assert_eq!(decremented.count, 1);
    assert_eq!(decremented.tags, ["a", "b"]);
    Ok(())
}

#[driver_test(
    id(ID),
    requires(upsert_primary_key),
    scenario(crate::scenarios::has_many_nullable_fk)
)]
pub async fn upsert_without_update_assignments_is_invalid(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User {}).exec(&mut db).await?;
    let error = User::upsert_by_id(seed.id).exec(&mut db).await.unwrap_err();
    assert!(error.is_invalid_statement(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(and(upsert_primary_key, not(upsert_branch_assignments))),
    scenario(crate::scenarios::two_models)
)]
pub async fn unsupported_upsert_branches_are_reported_before_dispatch(
    test: &mut Test,
) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let error = User::upsert_by_id(seed.id)
        .name("shared")
        .on_update(|update| update.name("updated"))
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(and(upsert_primary_key, not(upsert_unique))),
    scenario(crate::scenarios::user_unique_email_with_name)
)]
pub async fn unsupported_unique_upsert_is_reported_before_dispatch(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let error = User::upsert_by_email("alice@example.com")
        .name("Alice")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(not(upsert_primary_key)),
    scenario(crate::scenarios::two_models)
)]
pub async fn unsupported_upsert_is_reported_before_dispatch(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;
    let seed = toasty::create!(User { name: "seed" }).exec(&mut db).await?;
    let error = User::upsert_by_id(seed.id)
        .name("updated")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert!(error.is_unsupported_feature(), "unexpected error: {error}");
    Ok(())
}
