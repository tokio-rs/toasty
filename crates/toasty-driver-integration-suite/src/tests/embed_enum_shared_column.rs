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
            { name.app: Some("name"), shared: Some({ parts: ["name"] }) },
            { name.app: Some("profession"), shared: None },
            { name.app: Some("name"), shared: Some({ parts: ["name"] }) },
            { name.app: Some("species"), shared: None },
        ],
    }));
}

#[driver_test]
pub async fn raw_shared_identifier_uses_bare_name(t: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    enum Value {
        Text {
            #[shared(r#type)]
            kind: String,
        },
        Number {
            #[shared(r#type)]
            kind: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        id: String,
        value: Value,
    }

    let db = t.setup_db(models!(Record)).await;
    let schema = db.schema();

    assert_struct!(schema.app.models[&Value::id()], toasty::schema::app::Model::EmbeddedEnum({
        fields: [
            { shared: Some({ parts: ["type"] }) },
            { shared: Some({ parts: ["type"] }) },
        ],
    }));
    assert_struct!(schema.db.tables, [{
        columns: [
            { name: "id" },
            { name: "value" },
            { name: "value_type" },
        ],
    }]);
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

/// A variant-gated filter on a field that is not the variant's first keeps its
/// own column. The field's record position (2 here) equals the flattened index
/// of the Animal's shared `name` field, so the gateless shared read must not
/// claim the projection: `profession` filters `creature_profession`, not the
/// shared `creature_name` column. (Regression test: the shared read used to
/// lower `profession` to `creature_name`, silently matching nothing.)
#[driver_test(requires(scan), scenario(crate::scenarios::character_creature))]
pub async fn shared_column_variant_gated_second_field(t: &mut Test) -> Result<()> {
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
            name: "Rex".to_string(),
            species: "cat".to_string(),
        })
        .exec(&mut db)
        .await?;

    let builders = Character::filter(
        Character::fields()
            .creature()
            .human()
            .profession()
            .eq("builder"),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(builders.len(), 1);
    assert!(matches!(builders[0].creature, Creature::Human { .. }));

    let cats = Character::filter(Character::fields().creature().animal().species().eq("cat"))
        .exec(&mut db)
        .await?;
    assert_eq!(cats.len(), 1);
    assert!(matches!(cats[0].creature, Creature::Animal { .. }));

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

/// One gateless read of the shared column finds every variant at once, with
/// no per-variant OR.
#[driver_test(requires(scan), scenario(crate::scenarios::character_creature))]
pub async fn shared_column_gateless_filter(t: &mut Test) -> Result<()> {
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

    let bobs = Character::filter(Character::fields().creature().name().eq("Bob"))
        .exec(&mut db)
        .await?;
    assert_eq!(bobs.len(), 2);

    let not_bobs = Character::filter(Character::fields().creature().name().ne("Bob"))
        .exec(&mut db)
        .await?;
    assert_eq!(not_bobs.len(), 1);

    Ok(())
}

/// The gateless accessor orders by the single shared column.
#[driver_test(requires(sql), scenario(crate::scenarios::character_creature))]
pub async fn shared_column_gateless_order(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    for (name, profession) in [("Charlie", "builder"), ("Alice", "artist")] {
        Character::create()
            .creature(Creature::Human {
                name: name.to_string(),
                profession: profession.to_string(),
            })
            .exec(&mut db)
            .await?;
    }
    Character::create()
        .creature(Creature::Animal {
            name: "Bob".to_string(),
            species: "dog".to_string(),
        })
        .exec(&mut db)
        .await?;

    let ordered = Character::all()
        .order_by(Character::fields().creature().name().asc())
        .exec(&mut db)
        .await?;
    let names: Vec<String> = ordered
        .iter()
        .map(|c| match &c.creature {
            Creature::Human { name, .. } => name.clone(),
            Creature::Animal { name, .. } => name.clone(),
        })
        .collect();
    assert_eq!(names, ["Alice", "Bob", "Charlie"]);

    Ok(())
}

/// Rows whose variant does not declare the shared ident read `NULL`, and
/// ordering by the shared column follows the backend's `NULL` placement: the
/// `NULL` rows land at one end (first or last, per backend), while the
/// non-`NULL` rows stay ordered among themselves.
#[driver_test(requires(sql))]
pub async fn shared_column_gateless_order_null_placement(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        #[auto]
        id: uuid::Uuid,
        value: Value,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Value {
        #[column(variant = 1)]
        Text {
            #[shared(label)]
            label: String,
        },
        // `Number` declares no shared ident, so its rows read `NULL` for the
        // shared label column.
        #[column(variant = 2)]
        Number { n: i64 },
    }

    let mut db = t.setup_db(models!(Record)).await;

    Record::create()
        .value(Value::Text {
            label: "b".to_string(),
        })
        .exec(&mut db)
        .await?;
    Record::create()
        .value(Value::Number { n: 1 })
        .exec(&mut db)
        .await?;
    Record::create()
        .value(Value::Text {
            label: "a".to_string(),
        })
        .exec(&mut db)
        .await?;

    let ordered = Record::all()
        .order_by(Record::fields().value().label().asc())
        .exec(&mut db)
        .await?;
    let labels: Vec<Option<&str>> = ordered
        .iter()
        .map(|r| match &r.value {
            Value::Text { label } => Some(label.as_str()),
            Value::Number { .. } => None,
        })
        .collect();

    // The non-NULL rows sort among themselves...
    let non_null: Vec<&str> = labels.iter().filter_map(|l| *l).collect();
    assert_eq!(non_null, ["a", "b"]);

    // ...and every NULL row sits at one end of the result — which end is the
    // backend's choice, so accept either.
    let nulls_first = labels.first() == Some(&None);
    let nulls_last = labels.last() == Some(&None);
    assert!(
        nulls_first ^ nulls_last,
        "NULL rows must sort to exactly one end, got {labels:?}"
    );

    Ok(())
}

/// Rows whose variant does not declare the ident hold `NULL`: `eq` and `ne`
/// match neither, while `is_none()` / `is_some()` select on null-ness.
#[driver_test(requires(scan))]
pub async fn shared_column_gateless_null(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        #[auto]
        id: uuid::Uuid,
        value: Value,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Value {
        #[column(variant = 1)]
        Text {
            #[shared(label)]
            label: String,
        },
        #[column(variant = 2)]
        Number { n: i64 },
        #[column(variant = 3)]
        Other {
            #[shared(label)]
            label: String,
        },
        // Unit variants declare no fields, so they never participate in the
        // shared column and always read `NULL` for it.
        #[column(variant = 4)]
        Unit,
    }

    let mut db = t.setup_db(models!(Record)).await;

    Record::create()
        .value(Value::Text {
            label: "hi".to_string(),
        })
        .exec(&mut db)
        .await?;
    Record::create()
        .value(Value::Number { n: 1 })
        .exec(&mut db)
        .await?;
    Record::create().value(Value::Unit).exec(&mut db).await?;

    let his = Record::filter(Record::fields().value().label().eq("hi"))
        .exec(&mut db)
        .await?;
    assert_eq!(his.len(), 1);

    let not_his = Record::filter(Record::fields().value().label().ne("hi"))
        .exec(&mut db)
        .await?;
    assert_eq!(not_his.len(), 0);

    let nulls = Record::filter(Record::fields().value().label().is_none())
        .exec(&mut db)
        .await?;
    assert_eq!(nulls.len(), 2);

    let present = Record::filter(Record::fields().value().label().is_some())
        .exec(&mut db)
        .await?;
    assert_eq!(present.len(), 1);

    Ok(())
}

/// The gateless name is the shared ident, not any variant's Rust field name:
/// callers spell one path even when variants disagree on field names.
#[driver_test(requires(scan))]
pub async fn shared_column_gateless_distinct_names(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        #[auto]
        id: uuid::Uuid,
        creature: Creature,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            full_name: String,
            profession: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            nickname: String,
            species: String,
        },
    }

    let mut db = t.setup_db(models!(Character)).await;

    Character::create()
        .creature(Creature::Human {
            full_name: "Bob".to_string(),
            profession: "builder".to_string(),
        })
        .exec(&mut db)
        .await?;
    Character::create()
        .creature(Creature::Animal {
            nickname: "Bob".to_string(),
            species: "dog".to_string(),
        })
        .exec(&mut db)
        .await?;

    let bobs = Character::filter(Character::fields().creature().name().eq("Bob"))
        .exec(&mut db)
        .await?;
    assert_eq!(bobs.len(), 2);

    Ok(())
}

/// A variant-gated read's record position must never resolve as the gateless
/// shared read, even when a unit sibling variant lets the data variant span
/// every flattened field. The shared accessor offsets its step past
/// `fields.len()`, the largest reachable record position, so the gated
/// `kind` read keeps its own column.
///
/// Today the gated read cannot evaluate at all: distributing its record
/// position across a unit variant's one-slot record panics in the engine (the
/// same limitation exists on `main`, which panics earlier, at schema verify).
/// This test pins that panic. If the shared step ever claims the gated read's
/// record position again, this query stops panicking and silently sorts by
/// the shared `label` column instead — and this test fails. If the
/// record-position limitation is fixed first, update this test to assert
/// ordering by `kind`.
#[driver_test]
#[should_panic]
pub async fn shared_column_gated_order_by_unit_sibling(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    enum Probe {
        Alpha {
            #[shared(label)]
            label: String,
            kind: String,
        },
        Beta,
    }

    #[derive(Debug, toasty::Model)]
    struct Record {
        #[key]
        id: String,
        probe: Probe,
    }

    let mut db = t.setup_db(models!(Record)).await;

    // Ordered by `kind`, record "a" sorts first; ordered by the shared
    // `label` column, "b" would sort first.
    toasty::create!(Record {
        id: "a",
        probe: Probe::Alpha {
            label: "bbb".to_string(),
            kind: "aaa".to_string(),
        },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Record {
        id: "b",
        probe: Probe::Alpha {
            label: "aaa".to_string(),
            kind: "bbb".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    let ordered = Record::all()
        .order_by(Record::fields().probe().alpha().kind().asc())
        .exec(&mut db)
        .await?;
    let ids: Vec<_> = ordered.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["a", "b"]);

    Ok(())
}
