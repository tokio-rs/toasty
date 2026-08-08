use crate::prelude::*;

/// The polymorphic-owner shape: `#[belongs_to]` fields inside embedded enum
/// variants. The relation fields map to no columns — the discriminant and the
/// key fields own the storage. Creating supplies the variant value with
/// explicit keys, `match` reads the stored keys back, and the owner loads
/// with an ordinary `get_by_*`.
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
    let obj_a = Object::get_by_id(&mut db, obj_a.id).await?;
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

    Ok(())
}

/// Key fields of relation-carrying variants stay queryable through the
/// existing variant filter paths: the variant closure gates on the
/// discriminant and compares the key column.
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

    let human_obj = toasty::create!(Object {
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
    let found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|h| h.id().eq(alice.id)),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, human_obj.id);

    // The discriminant filter alone works as before.
    let humans: Vec<Object> = Object::filter(Object::fields().owner().is_human())
        .exec(&mut db)
        .await?;
    assert_eq!(humans.len(), 1);
    assert_eq!(humans[0].id, human_obj.id);

    Ok(())
}

/// Changing the owner — including its kind — is a whole-value replacement of
/// the embed, per existing embedded-enum update semantics.
#[driver_test]
pub async fn update_replaces_owner_variant(test: &mut Test) -> Result<()> {
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
    let bot = toasty::create!(Bot { serial: "B-1000" })
        .exec(&mut db)
        .await?;

    let mut obj = toasty::create!(Object {
        owner: Owner::Human {
            id: alice.id,
            human: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    obj.update()
        .owner(Owner::Bot {
            serial: bot.serial.clone(),
            bot: toasty::Deferred::default(),
        })
        .exec(&mut db)
        .await?;

    let reloaded = Object::get_by_id(&mut db, obj.id).await?;
    assert_struct!(
        reloaded.owner,
        Owner::Bot {
            serial: "B-1000",
            ..
        }
    );

    Ok(())
}

/// `#[belongs_to]` inside an embedded struct: same storage rule — the key
/// field owns the column, the relation maps to nothing.
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

    let post = Post::get_by_id(&mut db, post.id).await?;
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
    let mut post = post;
    post.update()
        .attribution(toasty::stmt::patch(
            Attribution::fields().author_id(),
            other.id,
        ))
        .exec(&mut db)
        .await?;
    assert_eq!(post.attribution.author_id, other.id);

    Ok(())
}

/// An `Option<Owner>` field: an ownerless row stores NULL in the discriminant
/// column, per existing optional-embed support.
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

    let orphan = Object::get_by_id(&mut db, orphan.id).await?;
    assert!(orphan.owner.is_none());

    let owned = Object::get_by_id(&mut db, owned.id).await?;
    match owned.owner {
        Some(Owner::Human { id, human: rel }) => {
            assert_eq!(id, human.id);
            assert!(rel.is_unloaded());
        }
        other => panic!("expected Some(Owner::Human), got {other:?}"),
    }

    Ok(())
}
