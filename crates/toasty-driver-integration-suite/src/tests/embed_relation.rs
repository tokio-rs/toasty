use crate::prelude::*;

/// The polymorphic-owner shape: `#[belongs_to]` fields inside embedded enum
/// variants, exercised through the full CRUD cycle. The relation fields map
/// to no columns — the discriminant and the key fields own the storage.
/// Creating supplies the variant value with explicit keys, `match` reads the
/// stored keys back, the owner loads with an ordinary `get_by_*`, and
/// changing the owner — including its kind — is a whole-value replacement of
/// the embed.
#[driver_test]
pub async fn belongs_to_in_enum_variants(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        serial: String,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Human {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Bot {
            #[index]
            serial: String,
            #[belongs_to(key = serial, references = serial)]
            bot: toasty::Deferred<Bot>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Object, Human, Bot)).await;

    // The relation fields contribute no columns: discriminant + one column
    // per key field.
    let table = &db.schema().db.tables[0];
    let names: Vec<_> = table.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "owner", "owner_id", "owner_serial"]);

    let alice = toasty::create!(Human { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bot = toasty::create!(Bot {
        serial: "B-1000",
        name: "Marvin"
    })
    .exec(&mut db)
    .await?;

    // Create with explicit keys; the relation stays unloaded.
    let obj_a = toasty::create!(Object {
        owner: Owner::Human {
            id: alice.id,
            human: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;
    let obj_b = toasty::create!(Object {
        owner: Owner::Bot {
            serial: bot.serial.clone(),
            bot: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    // `match` gives direct access to the stored keys; the owner loads with an
    // ordinary lookup.
    let mut obj_a = Object::get_by_id(&mut db, obj_a.id).await?;
    match &obj_a.owner {
        Owner::Human { id, human } => {
            assert!(human.is_unloaded());
            let human = Human::get_by_id(&mut db, id).await?;
            assert_eq!(human.name, "Alice");
        }
        other => panic!("expected Owner::Human, got {other:?}"),
    }

    let obj_b = Object::get_by_id(&mut db, obj_b.id).await?;
    match &obj_b.owner {
        Owner::Bot { serial, bot } => {
            assert!(bot.is_unloaded());
            let bot = Bot::get_by_serial(&mut db, serial).await?;
            assert_eq!(bot.name, "Marvin");
        }
        other => panic!("expected Owner::Bot, got {other:?}"),
    }

    // Changing the owner — including its kind — is a whole-value replacement
    // of the embed.
    obj_a
        .update()
        .owner(Owner::Bot {
            serial: bot.serial.clone(),
            bot: toasty::Deferred::default(),
        })
        .exec(&mut db)
        .await?;
    let reloaded = Object::get_by_id(&mut db, obj_a.id).await?;
    assert_struct!(
        reloaded.owner,
        Owner::Bot {
            serial: "B-1000",
            ..
        }
    );

    let obj_a_id = obj_a.id;
    obj_a.delete().exec(&mut db).await?;
    assert_err!(Object::get_by_id(&mut db, obj_a_id).await);
    assert!(matches!(
        Object::get_by_id(&mut db, obj_b.id).await?.owner,
        Owner::Bot { .. }
    ));

    Ok(())
}

/// Key fields of relation-carrying variants stay queryable through the
/// existing variant filter paths — the variant closure gates on the
/// discriminant and compares the shared key column — and stay consistent
/// as rows are re-pointed and deleted.
#[driver_test]
pub async fn filter_by_relation_key_through_variant_path(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Animal {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    // `Human` and `Animal` share one key column; `#[index(id)]` indexes it
    // once for both.
    #[derive(Debug, toasty::Embed)]
    #[index(id)]
    enum Owner {
        Human {
            #[shared(id)]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Animal {
            #[shared(id)]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            animal: toasty::Deferred<Animal>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Object, Human, Animal)).await;

    // One shared key column, indexed by the enum-level attribute.
    let table = &db.schema().db.tables[0];
    let names: Vec<_> = table.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "owner", "owner_id"]);
    let key_col = columns(&db, "objects", &["owner_id"])[0];
    assert_struct!(table.indices, [
        { primary_key: true },
        { unique: false, primary_key: false, columns: [{ column: == key_col }] },
    ]);

    let alice = toasty::create!(Human { name: "Alice" })
        .exec(&mut db)
        .await?;
    // An animal holding the same UUID as Alice, to prove the variant gate.
    let rex = toasty::create!(Animal { name: "Rex" })
        .exec(&mut db)
        .await?;

    let mut human_obj = toasty::create!(Object {
        owner: Owner::Human {
            id: alice.id,
            human: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object {
        owner: Owner::Animal {
            id: rex.id,
            animal: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    // Variant-gated key filter: only the Human row matches, even though the
    // Animal row stores its key in the same column.
    let by_key = |id: uuid::Uuid| {
        Object::fields()
            .owner()
            .human()
            .matches(move |h| h.id().eq(id))
    };
    let found: Vec<Object> = Object::filter(by_key(alice.id)).exec(&mut db).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, human_obj.id);

    // The discriminant filter alone works as before.
    let humans: Vec<Object> = Object::filter(Object::fields().owner().is_human())
        .exec(&mut db)
        .await?;
    assert_eq!(humans.len(), 1);
    assert_eq!(humans[0].id, human_obj.id);

    // Re-point the relation within the same variant by replacing the embed
    // value; the key filter follows.
    let bea = toasty::create!(Human { name: "Bea" }).exec(&mut db).await?;
    human_obj
        .update()
        .owner(Owner::Human {
            id: bea.id,
            human: toasty::Deferred::default(),
        })
        .exec(&mut db)
        .await?;
    assert!(
        Object::filter(by_key(alice.id))
            .exec(&mut db)
            .await?
            .is_empty()
    );
    let found: Vec<Object> = Object::filter(by_key(bea.id)).exec(&mut db).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, human_obj.id);

    // The relation accessor reaches the same row by model value.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().eq(&bea)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, human_obj.id);

    // Deleting the row empties the key filter; the Animal row is untouched.
    human_obj.delete().exec(&mut db).await?;
    assert!(
        Object::filter(by_key(bea.id))
            .exec(&mut db)
            .await?
            .is_empty()
    );
    let animals: Vec<Object> = Object::filter(Object::fields().owner().is_animal())
        .exec(&mut db)
        .await?;
    assert_eq!(animals.len(), 1);

    Ok(())
}

/// `#[belongs_to]` inside an embedded struct: same storage rule — the key
/// field owns the column, the relation maps to nothing — through the full
/// CRUD cycle.
#[driver_test]
pub async fn belongs_to_in_embedded_struct(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Author {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Attribution {
        #[index]
        author_id: uuid::Uuid,
        #[belongs_to(key = author_id)]
        author: toasty::Deferred<Author>,
        note: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        attribution: Attribution,
    }

    let mut db = test.setup_db(models!(Post, Author)).await;

    let table = &db.schema().db.tables[0];
    let names: Vec<_> = table.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "attribution_author_id", "attribution_note"]);

    let author = toasty::create!(Author { name: "Ann" })
        .exec(&mut db)
        .await?;

    let post = toasty::create!(Post {
        attribution: Attribution {
            author_id: author.id,
            author: toasty::Deferred::default(),
            note: "first draft".to_string(),
        }
    })
    .exec(&mut db)
    .await?;

    let mut post = Post::get_by_id(&mut db, post.id).await?;
    assert!(post.attribution.author.is_unloaded());
    assert_eq!(post.attribution.note, "first draft");
    let author = Author::get_by_id(&mut db, &post.attribution.author_id).await?;
    assert_eq!(author.name, "Ann");

    // The key field stays queryable through the embed path.
    let found: Vec<Post> = Post::filter(
        Post::fields()
            .attribution()
            .author_id()
            .eq(post.attribution.author_id),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);

    // And assignable through the embed update builder.
    let other = toasty::create!(Author { name: "Bea" })
        .exec(&mut db)
        .await?;
    post.update()
        .attribution(toasty::stmt::patch(
            Attribution::fields().author_id(),
            other.id,
        ))
        .exec(&mut db)
        .await?;
    assert_eq!(post.attribution.author_id, other.id);
    assert_eq!(
        Post::get_by_id(&mut db, post.id)
            .await?
            .attribution
            .author_id,
        other.id
    );

    let post_id = post.id;
    post.delete().exec(&mut db).await?;
    assert_err!(Post::get_by_id(&mut db, post_id).await);

    Ok(())
}

/// An `Option<Owner>` field: an ownerless row stores NULL in the discriminant
/// column, per existing optional-embed support, and updates move rows in and
/// out of ownership.
#[driver_test]
pub async fn optional_relation_carrying_embed(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Human {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Option<Owner>,
    }

    let mut db = test.setup_db(models!(Object, Human)).await;

    let orphan = toasty::create!(Object { owner: None })
        .exec(&mut db)
        .await?;

    let human = toasty::create!(Human {}).exec(&mut db).await?;
    let owned = toasty::create!(Object {
        owner: Some(Owner::Human {
            id: human.id,
            human: toasty::Deferred::default(),
        })
    })
    .exec(&mut db)
    .await?;

    let mut orphan = Object::get_by_id(&mut db, orphan.id).await?;
    assert!(orphan.owner.is_none());

    let mut owned = Object::get_by_id(&mut db, owned.id).await?;
    match &owned.owner {
        Some(Owner::Human { id, human: rel }) => {
            assert_eq!(*id, human.id);
            assert!(rel.is_unloaded());
        }
        other => panic!("expected Some(Owner::Human), got {other:?}"),
    }

    // Assign an owner to the ownerless row and clear the owned row.
    orphan
        .update()
        .owner(Some(Owner::Human {
            id: human.id,
            human: toasty::Deferred::default(),
        }))
        .exec(&mut db)
        .await?;
    assert!(matches!(
        Object::get_by_id(&mut db, orphan.id).await?.owner,
        Some(Owner::Human { .. })
    ));

    owned.update().owner(None).exec(&mut db).await?;
    assert!(Object::get_by_id(&mut db, owned.id).await?.owner.is_none());

    Ok(())
}

/// Setting an embedded relation from a parent model value. In `create!` and
/// `update!`, a variant literal passes the parent by reference and the key
/// field fills from the parent's referenced field — including a non-primary
/// `references = serial` key. In a plain (complete) literal, a loaded
/// relation value fills the key the same way, winning over the explicitly
/// written key.
#[driver_test]
pub async fn write_relation_from_parent_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[unique]
        serial: String,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Human {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Bot {
            #[index]
            serial: String,
            #[belongs_to(key = serial, references = serial)]
            bot: toasty::Deferred<Bot>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Object, Human, Bot)).await;

    let alice = toasty::create!(Human { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bot = toasty::create!(Bot {
        serial: "B-1000",
        name: "Marvin"
    })
    .exec(&mut db)
    .await?;

    // `create!` with the parent by reference; no explicit key.
    let obj = toasty::create!(Object {
        owner: Owner::Human { human: &alice }
    })
    .exec(&mut db)
    .await?;
    let mut obj = Object::get_by_id(&mut db, obj.id).await?;
    assert_struct!(obj.owner, Owner::Human { id: == alice.id, .. });

    // A non-primary `references` key fills from the referenced field, not
    // the parent's primary key.
    let obj_b = toasty::create!(Object {
        owner: Owner::Bot { bot: &bot }
    })
    .exec(&mut db)
    .await?;
    let obj_b = Object::get_by_id(&mut db, obj_b.id).await?;
    assert_struct!(obj_b.owner, Owner::Bot { serial: == bot.serial, .. });

    // `update!` changes the owner — including its kind — through the same
    // sugar.
    toasty::update!(obj {
        owner: Owner::Bot { bot: &bot }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(
        Object::get_by_id(&mut db, obj.id).await?.owner,
        Owner::Bot { serial: == bot.serial, .. }
    );

    // A loaded relation value in a complete literal fills the key too; the
    // written-out key loses to the parent value.
    let carol = toasty::create!(Human { name: "Carol" })
        .exec(&mut db)
        .await?;
    let carol_id = carol.id;
    let obj_c = toasty::create!(Object {
        owner: Owner::Human {
            id: uuid::Uuid::new_v4(),
            human: toasty::Deferred::from(carol),
        }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(
        Object::get_by_id(&mut db, obj_c.id).await?.owner,
        Owner::Human { id: == carol_id, .. }
    );

    Ok(())
}

/// A loaded relation value inside an embedded struct fills its key slot on
/// write, same as the enum-variant case — the struct's `IntoExpr` reads the
/// referenced field off the parent.
#[driver_test]
pub async fn write_struct_embed_relation_from_loaded_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Author {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Attribution {
        #[index]
        author_id: uuid::Uuid,
        #[belongs_to(key = author_id)]
        author: toasty::Deferred<Author>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        attribution: Attribution,
    }

    let mut db = test.setup_db(models!(Post, Author)).await;

    let ann = toasty::create!(Author { name: "Ann" })
        .exec(&mut db)
        .await?;
    let ann_id = ann.id;

    let post = toasty::create!(Post {
        attribution: Attribution {
            author_id: uuid::Uuid::new_v4(),
            author: toasty::Deferred::from(ann),
        }
    })
    .exec(&mut db)
    .await?;

    let post = Post::get_by_id(&mut db, post.id).await?;
    assert_eq!(post.attribution.author_id, ann_id);

    Ok(())
}

/// Filtering an embedded relation by model value. The comparison gates on
/// the variant's discriminant and compares the key column — a row of another
/// variant holding the same key in the shared column never matches — and
/// traversal into the target model lifts to a subquery behind the same gate.
#[driver_test]
pub async fn filter_by_relation_model_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[index]
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Animal {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    #[index(id)]
    enum Owner {
        Human {
            #[shared(id)]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Animal {
            #[shared(id)]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            animal: toasty::Deferred<Animal>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }

    let mut db = test.setup_db(models!(Object, Human, Animal)).await;

    let alice = toasty::create!(Human { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bea = toasty::create!(Human { name: "Bea" }).exec(&mut db).await?;

    let alice_obj = toasty::create!(Object {
        owner: Owner::Human { human: &alice }
    })
    .exec(&mut db)
    .await?;
    let bea_obj = toasty::create!(Object {
        owner: Owner::Human { human: &bea }
    })
    .exec(&mut db)
    .await?;
    // An Animal row holding *Alice's* uuid in the shared key column: the
    // proof that relation comparisons gate on the discriminant.
    toasty::create!(Object {
        owner: Owner::Animal {
            id: alice.id,
            animal: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    // eq by model value.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().eq(&alice)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, alice_obj.id);

    // The variant-scoped key path agrees with the model-value form.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.id().eq(alice.id)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, alice_obj.id);

    // Comparing against a different human matches only that human's row —
    // never the Animal row, whose shared key also differs from Bea's.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().eq(&bea)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, bea_obj.id);

    // Traversal into the target model.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().name().eq("Alice")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, alice_obj.id);

    // Subquery form.
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().in_query(Human::filter_by_name("Alice"))),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, alice_obj.id);

    Ok(())
}

/// Filtering a relation inside an embedded struct by model value: no
/// discriminant exists, the comparison resolves to the key column through
/// the embed path, and traversal lifts to a subquery.
#[driver_test]
pub async fn filter_struct_embed_relation_by_model_value(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Author {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
    }

    #[derive(Debug, toasty::Embed)]
    struct Attribution {
        #[index]
        author_id: uuid::Uuid,
        #[belongs_to(key = author_id)]
        author: toasty::Deferred<Author>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        attribution: Attribution,
    }

    let mut db = test.setup_db(models!(Post, Author)).await;

    let ann = toasty::create!(Author { name: "Ann" })
        .exec(&mut db)
        .await?;
    let bea = toasty::create!(Author { name: "Bea" })
        .exec(&mut db)
        .await?;

    let ann_post = toasty::create!(Post {
        attribution: Attribution {
            author_id: ann.id,
            author: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;
    let bea_post = toasty::create!(Post {
        attribution: Attribution {
            author_id: bea.id,
            author: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    let found: Vec<Post> = Post::filter(Post::fields().attribution().author().eq(&ann))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, ann_post.id);

    let found: Vec<Post> = Post::filter(Post::fields().attribution().author().name().eq("Bea"))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, bea_post.id);

    Ok(())
}
