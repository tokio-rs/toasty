use crate::prelude::*;
use toasty::stmt::IntoExpr;

/// Comparing two relations that live in enum variants requires *both*
/// variants. Every variant here stores its key in the same shared column,
/// so without both checks a row holding `Other` on either side would match
/// on key equality alone. `ne` keeps both checks as well, and negating an
/// equality negates the guarded comparison as a whole.
#[driver_test]
pub async fn compare_embedded_relations_preserves_both_variant_guards(
    test: &mut Test,
) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }

    #[derive(Debug, toasty::Embed)]
    #[index(id)]
    enum Owner {
        Primary {
            #[shared(id)]
            id: uuid::Uuid,
            #[shared(active)]
            active: bool,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Other {
            #[shared(id)]
            id: uuid::Uuid,
            #[shared(active)]
            active: bool,
            #[belongs_to(key = id)]
            other: toasty::Deferred<Human>,
        },
    }

    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        lhs: Owner,
        rhs: Owner,
    }

    let mut db = test.setup_db(models!(Object, Human)).await;
    let ann = toasty::create!(Human {}).exec(&mut db).await?;
    let bea = toasty::create!(Human {}).exec(&mut db).await?;

    // Eight rows: every combination of variant on each side, with the two
    // keys equal or not. Only the row with Primary on both sides and equal
    // keys satisfies the equality; only the Primary/Primary row with
    // different keys satisfies the inequality.
    let mut equal = None;
    let mut unequal = None;
    for left_primary in [true, false] {
        for right_primary in [true, false] {
            for same in [true, false] {
                let right = if same { &ann } else { &bea };
                let lhs = if left_primary {
                    Owner::Primary {
                        id: ann.id,
                        active: true,
                        human: toasty::Deferred::default(),
                    }
                } else {
                    Owner::Other {
                        id: ann.id,
                        active: true,
                        other: toasty::Deferred::default(),
                    }
                };
                let rhs = if right_primary {
                    Owner::Primary {
                        id: right.id,
                        active: same,
                        human: toasty::Deferred::default(),
                    }
                } else {
                    Owner::Other {
                        id: right.id,
                        active: same,
                        other: toasty::Deferred::default(),
                    }
                };
                let object = toasty::create!(Object { lhs, rhs }).exec(&mut db).await?;
                if left_primary && right_primary {
                    if same {
                        equal = Some(object.id);
                    } else {
                        unequal = Some(object.id);
                    }
                }
            }
        }
    }

    let lhs = || Object::fields().lhs().primary();
    let rhs = || Object::fields().rhs().primary();
    for (predicate, expected) in [
        // Relation against relation, in both directions.
        (lhs().human().eq(rhs().human()), equal),
        (rhs().human().eq(lhs().human()), equal),
        (lhs().human().ne(rhs().human()), unequal),
        (rhs().human().ne(lhs().human()), unequal),
        // The key fields themselves.
        (lhs().id().eq(rhs().id()), equal),
        (rhs().id().eq(lhs().id()), equal),
        (lhs().id().ne(rhs().id()), unequal),
        (rhs().id().ne(lhs().id()), unequal),
        // A path converted to an expression first.
        (lhs().human().into_expr().eq(rhs().human()), equal),
        (lhs().id().into_expr().ne(rhs().id()), unequal),
    ] {
        let found = Object::filter(predicate).exec(&mut db).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(Some(found[0].id), expected);
    }

    // Negating the equality negates the whole guarded comparison: every row
    // but the equal Primary/Primary one.
    let found = Object::filter(lhs().human().eq(rhs().human()).not())
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 7);

    // A single-side predicate on a shared column still requires its variant.
    let found = Object::filter(lhs().active().eq(true))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 4);
    let found = Object::filter(rhs().active().eq(false))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 2);

    Ok(())
}

/// A composite-key relation inside a variant of an enum that is itself a
/// variant field of an outer enum, shared by the nested-variant tests.
mod nested_composite {
    use crate::prelude::*;

    #[derive(Debug, toasty::Model)]
    #[key(partition = namespace, local = revision)]
    pub struct Parent {
        pub namespace: uuid::Uuid,
        pub revision: i64,
        pub name: String,
    }

    #[derive(Debug, toasty::Embed)]
    pub enum Owner {
        Parent {
            #[index]
            namespace: uuid::Uuid,
            revision: i64,
            #[belongs_to(key = [namespace, revision], references = [namespace, revision])]
            parent: toasty::Deferred<Parent>,
        },
        Empty,
    }

    #[derive(Debug, toasty::Embed)]
    pub enum Envelope {
        First { owner: Owner },
        Second { other_owner: Owner },
    }

    #[derive(Debug, toasty::Model)]
    pub struct Object {
        #[key]
        #[auto]
        pub id: uuid::Uuid,
        pub envelope: Envelope,
    }

    /// The rows the tests query. `Second { other_owner }` stores the same
    /// keys in other columns; it must never match a predicate through
    /// `first().owner()`. `parents` are `first`, `second` (same namespace),
    /// and `third` (another namespace). `objects` are the `First` objects
    /// owned by each of them, in the same order.
    pub struct Fixture {
        pub parents: [Parent; 3],
        pub objects: [Object; 3],
    }

    /// The nested literal is written out with its keys: `create!` fills
    /// keys from a parent value only for the outer variant literal.
    pub fn owner(parent: &Parent) -> Owner {
        Owner::Parent {
            namespace: parent.namespace,
            revision: parent.revision,
            parent: toasty::Deferred::default(),
        }
    }

    pub async fn setup(test: &mut Test) -> Result<(toasty::Db, Fixture)> {
        let mut db = test.setup_db(models!(Object, Parent)).await;
        let namespace = uuid::Uuid::new_v4();
        let mut parents = Vec::with_capacity(3);
        for (namespace, revision, name) in [
            (namespace, 1, "first"),
            (namespace, 2, "second"),
            (uuid::Uuid::new_v4(), 2, "third"),
        ] {
            parents.push(
                toasty::create!(Parent {
                    namespace,
                    revision,
                    name
                })
                .exec(&mut db)
                .await?,
            );
        }

        let mut objects = Vec::with_capacity(3);
        for parent in &parents {
            objects.push(
                toasty::create!(Object {
                    envelope: Envelope::First {
                        owner: owner(parent)
                    },
                })
                .exec(&mut db)
                .await?,
            );
        }
        toasty::create!(Object {
            envelope: Envelope::Second {
                other_owner: owner(&parents[0])
            },
        })
        .exec(&mut db)
        .await?;
        toasty::create!(Object {
            envelope: Envelope::First {
                owner: Owner::Empty
            }
        })
        .exec(&mut db)
        .await?;

        let fixture = Fixture {
            parents: parents.try_into().unwrap(),
            objects: objects.try_into().unwrap(),
        };
        Ok((db, fixture))
    }
}

/// Every predicate through the nested path requires both variants, whether
/// the comparison is at the relation, a path converted through `IntoExpr`,
/// or a check of the inner variant.
#[driver_test]
pub async fn nested_variant_composite_relation(test: &mut Test) -> Result<()> {
    use nested_composite::{Envelope, Object, Owner, Parent, owner};

    let (mut db, fixture) = nested_composite::setup(test).await?;
    let [first, second, _] = &fixture.parents;
    let [object, ..] = &fixture.objects;
    let variant = || Object::fields().envelope().first();

    // A model value against the relation, and the same through a path
    // converted with `IntoExpr`.
    let found = Object::filter(variant().owner().parent().parent().eq(first))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);

    let path: toasty::stmt::Path<Object, Parent> = variant().owner().parent().parent().into();
    let found = Object::filter(path.into_expr().eq(first))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);

    // The inner variant check on its own requires the outer variant too.
    let found = Object::filter(variant().owner().is_parent())
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 3);
    let found = Object::filter(variant().owner().is_empty())
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);

    // Replacing the nested owner writes the new keys through both variants.
    let mut object = Object::get_by_id(&mut db, object.id).await?;
    toasty::update!(object {
        envelope: Envelope::First {
            owner: owner(second)
        }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(
        Object::get_by_id(&mut db, object.id).await?.envelope,
        Envelope::First {
            owner: Owner::Parent { revision: 2, .. }
        }
    );

    Ok(())
}

/// Traversal into the target of the nested composite-key relation. A filter
/// through a composite foreign key lifts to a tuple `IN` subquery, which
/// only the SQL backends evaluate.
#[driver_test(requires(sql))]
pub async fn nested_variant_composite_relation_traversal(test: &mut Test) -> Result<()> {
    use nested_composite::Object;

    let (mut db, fixture) = nested_composite::setup(test).await?;
    let [object, _, third_object] = &fixture.objects;
    let variant = || Object::fields().envelope().first();

    let found = Object::filter(
        variant()
            .owner()
            .parent()
            .matches(|v| v.parent().name().eq("first")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);

    let found = Object::filter(
        variant()
            .owner()
            .parent()
            .matches(|v| v.parent().name().ne("second")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|row| row.id == object.id));
    assert!(found.iter().any(|row| row.id == third_object.id));

    let found = Object::filter(
        variant()
            .owner()
            .parent()
            .matches(|v| v.parent().name().eq("missing")),
    )
    .exec(&mut db)
    .await?;
    assert!(found.is_empty());

    Ok(())
}

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

/// List membership on a relation inside an embedded enum variant: the
/// relation resolves to its variant-scoped key column, so an Animal row
/// holding one of the listed humans' ids in the shared key column does not
/// match.
#[driver_test]
pub async fn filter_enum_embed_relation_in_list(test: &mut Test) -> Result<()> {
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
    let cid = toasty::create!(Human { name: "Cid" }).exec(&mut db).await?;

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
    toasty::create!(Object {
        owner: Owner::Human { human: &cid }
    })
    .exec(&mut db)
    .await?;
    // An Animal row holding Alice's uuid in the shared key column.
    toasty::create!(Object {
        owner: Owner::Animal {
            id: alice.id,
            animal: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    let mut found: Vec<Object> = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| toasty::stmt::Expr::in_list(v.human(), [&alice, &bea])),
    )
    .exec(&mut db)
    .await?;
    found.sort_by_key(|o| o.id);

    let mut expected = [alice_obj.id, bea_obj.id];
    expected.sort();
    assert_struct!(found, [_ { id: == expected[0], .. }, _ { id: == expected[1], .. }]);

    Ok(())
}

/// List membership on a relation inside an embedded struct resolves to the
/// key column through the embed path.
#[driver_test]
pub async fn filter_struct_embed_relation_in_list(test: &mut Test) -> Result<()> {
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

    let alice = toasty::create!(Author { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bea = toasty::create!(Author { name: "Bea" })
        .exec(&mut db)
        .await?;
    let cid = toasty::create!(Author { name: "Cid" })
        .exec(&mut db)
        .await?;

    let alice_post = toasty::create!(Post {
        attribution: Attribution {
            author_id: alice.id,
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
    toasty::create!(Post {
        attribution: Attribution {
            author_id: cid.id,
            author: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    let mut found: Vec<Post> = Post::filter(toasty::stmt::Expr::in_list(
        Post::fields().attribution().author(),
        [&alice, &bea],
    ))
    .exec(&mut db)
    .await?;
    found.sort_by_key(|p| p.id);

    let mut expected = [alice_post.id, bea_post.id];
    expected.sort();
    assert_struct!(found, [_ { id: == expected[0], .. }, _ { id: == expected[1], .. }]);

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

/// Comparing one embedded relation to another substitutes the key on both
/// sides of the comparison. Rewriting only one side would leave the other
/// as the relation's storage-less record slot, which lowers to `Null` and
/// matches nothing.
#[driver_test]
pub async fn compare_embedded_relation_to_embedded_relation(test: &mut Test) -> Result<()> {
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

    #[derive(Debug, toasty::Embed)]
    struct Review {
        #[index]
        reviewer_id: uuid::Uuid,
        #[belongs_to(key = reviewer_id)]
        reviewer: toasty::Deferred<Author>,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        attribution: Attribution,
        review: Review,
    }

    let mut db = test.setup_db(models!(Post, Author)).await;

    let ann = toasty::create!(Author { name: "Ann" })
        .exec(&mut db)
        .await?;
    let bea = toasty::create!(Author { name: "Bea" })
        .exec(&mut db)
        .await?;

    // Ann reviewed her own post; Ann also reviewed Bea's post.
    let self_reviewed = toasty::create!(Post {
        attribution: Attribution {
            author_id: ann.id,
            author: toasty::Deferred::default(),
        },
        review: Review {
            reviewer_id: ann.id,
            reviewer: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Post {
        attribution: Attribution {
            author_id: bea.id,
            author: toasty::Deferred::default(),
        },
        review: Review {
            reviewer_id: ann.id,
            reviewer: toasty::Deferred::default(),
        }
    })
    .exec(&mut db)
    .await?;

    let found: Vec<Post> = Post::filter(
        Post::fields()
            .attribution()
            .author()
            .eq(Post::fields().review().reviewer()),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, self_reviewed.id);

    Ok(())
}
