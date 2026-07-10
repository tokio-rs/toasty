use crate::prelude::*;

/// An enum-level `#[unique(name)]` referencing a shared logical field produces
/// one unique DB index on the single shared column.
#[driver_test]
pub async fn shared_field_unique_index_schema(test: &mut Test) {
    #[derive(Debug, toasty::Embed)]
    #[unique(name)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            name: String,
            #[allow(dead_code)]
            profession: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            name: String,
            #[allow(dead_code)]
            species: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        id: String,
        #[allow(dead_code)]
        creature: Creature,
    }

    let db = test.setup_db(models!(Character)).await;
    let schema = db.schema();

    let table = &schema.db.tables[0];
    let name_col = columns(&db, "characters", &["creature_name"])[0];

    assert_struct!(table.indices, [
        { primary_key: true },
        { unique: true, primary_key: false, columns: [{ column: == name_col }] },
    ]);
}

/// Uniqueness on a shared column is cross-variant: a `Human` and an `Animal`
/// with the same name conflict, matching the shared column's un-gated query
/// semantics. Distinct names on either variant are accepted.
#[driver_test]
pub async fn shared_field_unique_enforced_cross_variant(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[unique(name)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            name: String,
            profession: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            name: String,
            species: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        id: String,
        creature: Creature,
    }

    let mut db = test.setup_db(models!(Character)).await;

    toasty::create!(Character {
        id: "1",
        creature: Creature::Human {
            name: "Bob".to_string(),
            profession: "builder".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    // An Animal named "Bob" hits the same shared column value — rejected.
    assert_err!(
        toasty::create!(Character {
            id: "2",
            creature: Creature::Animal {
                name: "Bob".to_string(),
                species: "dog".to_string(),
            }
        })
        .exec(&mut db)
        .await
    );

    // A different name on either variant is fine.
    toasty::create!(Character {
        id: "3",
        creature: Creature::Animal {
            name: "Rex".to_string(),
            species: "dog".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Character {
        id: "4",
        creature: Creature::Human {
            name: "Alice".to_string(),
            profession: "artist".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    Ok(())
}

/// A non-unique enum-level `#[index(name)]` on a shared field produces a
/// non-unique index and permits duplicate values across variants.
#[driver_test]
pub async fn shared_field_non_unique_index(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[index(name)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            name: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            name: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        id: String,
        creature: Creature,
    }

    let mut db = test.setup_db(models!(Character)).await;

    let name_col = columns(&db, "characters", &["creature_name"])[0];
    assert_struct!(db.schema().db.tables[0].indices, [
        { primary_key: true },
        { unique: false, primary_key: false, columns: [{ column: == name_col }] },
    ]);

    // Duplicates across variants are allowed on a non-unique index.
    toasty::create!(Character {
        id: "1",
        creature: Creature::Human {
            name: "Bob".to_string()
        }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Character {
        id: "2",
        creature: Creature::Animal {
            name: "Bob".to_string()
        }
    })
    .exec(&mut db)
    .await?;

    Ok(())
}

/// An enum-level attribute may reference a variant field that owns its column
/// via a `variant::field` path; combined with a shared field it produces a
/// composite index. Composite unique indices are SQL-only (DynamoDB does not
/// support them; see `index_composite`).
#[driver_test(requires(sql))]
pub async fn composite_index_shared_and_variant_field(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[unique(name, human::profession)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            name: String,
            profession: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            name: String,
            species: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        id: String,
        creature: Creature,
    }

    let mut db = test.setup_db(models!(Character)).await;

    let cols = columns(&db, "characters", &["creature_name", "creature_profession"]);
    assert_struct!(db.schema().db.tables[0].indices, [
        { primary_key: true },
        {
            unique: true,
            primary_key: false,
            columns: [{ column: == cols[0] }, { column: == cols[1] }],
        },
    ]);

    toasty::create!(Character {
        id: "1",
        creature: Creature::Human {
            name: "Bob".to_string(),
            profession: "builder".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    // Same (name, profession) combination is rejected.
    assert_err!(
        toasty::create!(Character {
            id: "2",
            creature: Creature::Human {
                name: "Bob".to_string(),
                profession: "builder".to_string(),
            }
        })
        .exec(&mut db)
        .await
    );

    // Same name, different profession — allowed; uniqueness is on the pair.
    toasty::create!(Character {
        id: "3",
        creature: Creature::Human {
            name: "Bob".to_string(),
            profession: "architect".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    Ok(())
}

/// Two variant fields resolving to the same column name without a common
/// `#[shared]` identifier are a duplicate column, not an implicit merge. The
/// macro cannot catch this case (for embedded fields `#[column]` is a
/// flattening prefix, not a single column), so the schema builder rejects it.
#[driver_test]
pub async fn duplicate_column_without_shared_rejected(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    enum Value {
        #[column(variant = 1)]
        Text {
            #[allow(dead_code)]
            #[column("value")]
            text: String,
        },
        #[column(variant = 2)]
        Number {
            #[allow(dead_code)]
            #[column("value")]
            number: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Holder {
        #[key]
        id: String,
        #[allow(dead_code)]
        value: Value,
    }

    let err = assert_err!(test.try_setup_db(models!(Holder)).await);
    assert!(
        err.to_string().contains("without declaring a shared field"),
        "unexpected error: {err}"
    );

    Ok(())
}

/// Columns created while mapping a nested enum stay in the outer enum's
/// namespace: a later variant field of the outer enum whose flattened name
/// collides with one of the nested enum's columns is a duplicate, not a
/// silent second column.
#[driver_test]
pub async fn nested_enum_column_collision_rejected(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Inner {
        #[column(variant = 1)]
        A {
            #[allow(dead_code)]
            x: String,
        },
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Outer {
        #[column(variant = 1)]
        V1 {
            #[allow(dead_code)]
            inner: Inner,
        },
        // Flattens to the same column as `Inner::A::x` above
        // (`{field}_inner_x`).
        #[column(variant = 2)]
        V2 {
            #[allow(dead_code)]
            inner_x: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Holder {
        #[key]
        id: String,
        #[allow(dead_code)]
        value: Outer,
    }

    let err = assert_err!(test.try_setup_db(models!(Holder)).await);
    assert!(
        err.to_string().contains("without declaring a shared field"),
        "unexpected error: {err}"
    );

    Ok(())
}

/// Same collision as above, opposite order: the plain variant field comes
/// first and the nested enum second, so the nested enum's variant fields must
/// be checked against column names the outer enum already created.
#[driver_test]
pub async fn nested_enum_column_collision_rejected_reverse_order(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Inner {
        #[column(variant = 1)]
        A {
            #[allow(dead_code)]
            x: String,
        },
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Outer {
        // Flattens to the same column as `Inner::A::x` below
        // (`{field}_inner_x`).
        #[column(variant = 1)]
        V1 {
            #[allow(dead_code)]
            inner_x: String,
        },
        #[column(variant = 2)]
        V2 {
            #[allow(dead_code)]
            inner: Inner,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Holder {
        #[key]
        id: String,
        #[allow(dead_code)]
        value: Outer,
    }

    let err = assert_err!(test.try_setup_db(models!(Holder)).await);
    assert!(
        err.to_string().contains("without declaring a shared field"),
        "unexpected error: {err}"
    );

    Ok(())
}

/// Embedded-struct variant fields sharing a `#[column("...")]` *prefix* are
/// not a collision: the string prefixes each flattened leaf, so differently
/// shaped embeds produce disjoint columns.
#[driver_test]
pub async fn embedded_prefix_collision_is_not_duplicate(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    struct Geo {
        lat: String,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    struct Postal {
        zip: String,
    }

    #[derive(Debug, PartialEq, toasty::Embed)]
    enum Location {
        #[column(variant = 1)]
        Coords {
            #[column("common")]
            geo: Geo,
        },
        #[column(variant = 2)]
        Mail {
            #[column("common")]
            postal: Postal,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Place {
        #[key]
        id: String,
        location: Location,
    }

    let mut db = test.setup_db(models!(Place)).await;

    // Disjoint flattened columns, no duplicate-column error.
    let table = &db.schema().db.tables[0];
    assert!(
        table
            .columns
            .iter()
            .any(|c| c.name == "location_common_lat")
    );
    assert!(
        table
            .columns
            .iter()
            .any(|c| c.name == "location_common_zip")
    );

    toasty::create!(Place {
        id: "1",
        location: Location::Coords {
            geo: Geo {
                lat: "47.6".to_string()
            }
        }
    })
    .exec(&mut db)
    .await?;

    let place = Place::get_by_id(&mut db, &"1".to_string()).await?;
    assert!(matches!(place.location, Location::Coords { .. }));

    Ok(())
}

/// A `#[column("...")]` override on a shared group renames the shared column;
/// the shared identifier keeps naming the field in `#[unique(...)]`, and the
/// index lands on the renamed column. The override needs declaring on only one
/// member of the group.
#[driver_test]
pub async fn shared_field_column_override(test: &mut Test) -> Result<()> {
    #[derive(Debug, PartialEq, toasty::Embed)]
    #[unique(name)]
    enum Creature {
        #[column(variant = 1)]
        Human {
            #[shared(name)]
            #[column("legacy_name")]
            name: String,
        },
        #[column(variant = 2)]
        Animal {
            #[shared(name)]
            name: String,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Character {
        #[key]
        id: String,
        creature: Creature,
    }

    let mut db = test.setup_db(models!(Character)).await;

    // One shared column under the overridden name; no `creature_name` column.
    let table = &db.schema().db.tables[0];
    assert!(
        table
            .columns
            .iter()
            .any(|c| c.name == "creature_legacy_name")
    );
    assert!(!table.columns.iter().any(|c| c.name == "creature_name"));

    let legacy_col = columns(&db, "characters", &["creature_legacy_name"])[0];
    assert_struct!(table.indices, [
        { primary_key: true },
        { unique: true, primary_key: false, columns: [{ column: == legacy_col }] },
    ]);

    // Both variants round-trip through the renamed shared column.
    toasty::create!(Character {
        id: "1",
        creature: Creature::Human {
            name: "Bob".to_string()
        }
    })
    .exec(&mut db)
    .await?;

    assert_struct!(
        Character::get_by_id(&mut db, &"1".to_string()).await?,
        _ { creature: Creature::Human { name: "Bob", .. }, .. }
    );

    Ok(())
}
