use crate::prelude::*;

/// A field declared `#[shared(name)]` in two variants coalesces into a single
/// shared, nullable column rather than producing one column per variant. The
/// table therefore has exactly one `creature_name` column alongside each
/// variant's own distinct column.
#[driver_test(scenario(crate::scenarios::character_creature))]
pub async fn shared_column_db_schema(t: &mut Test) {
    let db = setup(t).await;
    let schema = db.schema();

    assert_struct!(schema.db.tables, [
        {
            name: =~ r"characters$",
            columns: [
                { name: "id" },
                { name: "creature", nullable: false },
                // Shared by both Human and Animal — present exactly once.
                { name: "creature_name", nullable: true },
                { name: "creature_profession", nullable: true },
                { name: "creature_species", nullable: true },
            ],
        },
    ]);
}

/// The `#[shared(name)]` declaration surfaces in the app schema as the field's
/// shared identifier; both variants' `name` fields carry the same identifier,
/// which is what drives the column coalescing.
#[driver_test(scenario(crate::scenarios::character_creature))]
pub async fn shared_column_schema_fields(t: &mut Test) {
    let db = setup(t).await;
    let schema = db.schema();

    let creature = &schema.app.models[&Creature::id()];
    assert_struct!(creature, toasty::schema::app::Model::EmbeddedEnum({
        fields: [
            { name.app: Some("name"), shared: Some("name") },
            { name.app: Some("profession"), shared: None },
            { name.app: Some("name"), shared: Some("name") },
            { name.app: Some("species"), shared: None },
        ],
    }));
}

/// Both variants write and read the shared column, while their variant-specific
/// columns round-trip independently.
#[driver_test(scenario(crate::scenarios::character_creature))]
pub async fn shared_column_roundtrip(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let human = Character::create()
        .creature(Creature::Human {
            name: "Alice".to_string(),
            profession: "engineer".to_string(),
        })
        .exec(&mut db)
        .await?;

    let animal = Character::create()
        .creature(Creature::Animal {
            name: "Rex".to_string(),
            species: "dog".to_string(),
        })
        .exec(&mut db)
        .await?;

    assert_eq!(
        Character::get_by_id(&mut db, &human.id).await?.creature,
        Creature::Human {
            name: "Alice".to_string(),
            profession: "engineer".to_string(),
        }
    );
    assert_eq!(
        Character::get_by_id(&mut db, &animal.id).await?.creature,
        Creature::Animal {
            name: "Rex".to_string(),
            species: "dog".to_string(),
        }
    );

    Ok(())
}

/// Updating the whole enum field — including switching variants — re-encodes the
/// shared column correctly. The merged per-variant encode must select the arm
/// matching the *new* discriminant, so the shared column follows the value into
/// its new variant while the old variant's column is cleared to NULL.
#[driver_test(scenario(crate::scenarios::character_creature))]
pub async fn shared_column_update(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let mut character = Character::create()
        .creature(Creature::Human {
            name: "Bob".to_string(),
            profession: "builder".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Update within the same variant: only the shared column and the Human
    // column change.
    character
        .update()
        .creature(Creature::Human {
            name: "Bobby".to_string(),
            profession: "architect".to_string(),
        })
        .exec(&mut db)
        .await?;

    assert_eq!(
        Character::get_by_id(&mut db, &character.id).await?.creature,
        Creature::Human {
            name: "Bobby".to_string(),
            profession: "architect".to_string(),
        }
    );

    // Switch variant: the shared `creature_name` column now holds the Animal's
    // name, the Human column is cleared, and the Animal column is populated.
    character
        .update()
        .creature(Creature::Animal {
            name: "Whiskers".to_string(),
            species: "cat".to_string(),
        })
        .exec(&mut db)
        .await?;

    assert_eq!(
        Character::get_by_id(&mut db, &character.id).await?.creature,
        Creature::Animal {
            name: "Whiskers".to_string(),
            species: "cat".to_string(),
        }
    );

    Ok(())
}

// Mismatched shared-column types are rejected at compile time by the
// `SameColumnType` obligation the `Embed` derive emits; see the trybuild case
// `tests/ui/enum_shared_column_type_mismatch.rs`.

/// Both variants store their `name` in the same physical `creature_name`
/// column. A variant-rooted filter on that column keeps its implicit variant
/// gate, so `human().name().eq("Bob")` matches only Human rows even though an
/// Animal stores the same value in the same column — the discriminant
/// disambiguates the shared column per variant.
#[driver_test(requires(scan), scenario(crate::scenarios::character_creature))]
pub async fn shared_column_variant_gated_filter(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    for (name, profession) in [("Bob", "builder"), ("Alice", "artist")] {
        Character::create()
            .creature(Creature::Human {
                name: name.to_string(),
                profession: profession.to_string(),
            })
            .exec(&mut db)
            .await?;
    }

    for (name, species) in [("Bob", "dog"), ("Rex", "cat")] {
        Character::create()
            .creature(Creature::Animal {
                name: name.to_string(),
                species: species.to_string(),
            })
            .exec(&mut db)
            .await?;
    }

    // "Bob" lives in `creature_name` for both a Human and an Animal. The gate on
    // the Human-rooted filter must read that shared column yet exclude the
    // Animal that shares its value.
    let human_bobs = Character::filter(Character::fields().creature().human().name().eq("Bob"))
        .exec(&mut db)
        .await?;
    assert_eq!(human_bobs.len(), 1);
    assert!(matches!(human_bobs[0].creature, Creature::Human { .. }));

    // The same shared column, gated to the Animal variant, finds the Animal
    // "Bob" — proving the one column genuinely holds both variants' names.
    let animal_bobs = Character::filter(Character::fields().creature().animal().name().eq("Bob"))
        .exec(&mut db)
        .await?;
    assert_eq!(animal_bobs.len(), 1);
    assert!(matches!(animal_bobs[0].creature, Creature::Animal { .. }));

    // A name only one variant uses still resolves correctly through the gate.
    let humans_named_alice =
        Character::filter(Character::fields().creature().human().name().eq("Alice"))
            .exec(&mut db)
            .await?;
    assert_eq!(humans_named_alice.len(), 1);

    Ok(())
}

/// OR-ing the two variant-gated predicates on the shared `creature_name` column
/// is the natural way to query a single shared column across variants: "any
/// creature named Bob, regardless of variant". This used to panic in the SQL
/// serializer (issue #1061) because factoring lifted the shared predicate out
/// from under its variant gates, exposing the decode's unreachable `Error` else
/// branch.
#[driver_test(requires(scan), scenario(crate::scenarios::character_creature))]
pub async fn shared_column_cross_variant_or(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    Character::create()
        .creature(Creature::Human {
            name: "Bob".to_string(),
            profession: "builder".to_string(),
        })
        .exec(&mut db)
        .await?;
    Character::create()
        .creature(Creature::Animal {
            name: "Bob".to_string(),
            species: "dog".to_string(),
        })
        .exec(&mut db)
        .await?;
    Character::create()
        .creature(Creature::Animal {
            name: "Rex".to_string(),
            species: "cat".to_string(),
        })
        .exec(&mut db)
        .await?;

    // Both a Human "Bob" and an Animal "Bob" live in the shared column; the
    // cross-variant OR finds both while excluding "Rex".
    let bobs = Character::filter(
        Character::fields()
            .creature()
            .human()
            .name()
            .eq("Bob")
            .or(Character::fields().creature().animal().name().eq("Bob")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(bobs.len(), 2);

    Ok(())
}
