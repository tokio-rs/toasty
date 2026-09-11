use crate::prelude::*;

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
            #[belongs_to(key = id)]
            human: toasty::Deferred<Human>,
        },
        Other {
            #[shared(id)]
            id: uuid::Uuid,
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
    let mut equal = None;
    let mut unequal = None;
    // All variant combinations use the same shared key columns. Only rows
    // with Primary on both sides satisfy either relation comparison.
    for left_primary in [true, false] {
        for right_primary in [true, false] {
            for same in [true, false] {
                let right = if same { &ann } else { &bea };
                let lhs = if left_primary {
                    Owner::Primary {
                        id: ann.id,
                        human: toasty::Deferred::default(),
                    }
                } else {
                    Owner::Other {
                        id: ann.id,
                        other: toasty::Deferred::default(),
                    }
                };
                let rhs = if right_primary {
                    Owner::Primary {
                        id: right.id,
                        human: toasty::Deferred::default(),
                    }
                } else {
                    Owner::Other {
                        id: right.id,
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
    let lhs = || Object::fields().lhs().primary().human();
    let rhs = || Object::fields().rhs().primary().human();
    for (predicate, expected) in [
        (lhs().eq(rhs()), equal),
        (rhs().eq(lhs()), equal),
        (lhs().ne(rhs()), unequal),
        (rhs().ne(lhs()), unequal),
    ] {
        let found = Object::filter(predicate).exec(&mut db).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(Some(found[0].id), expected);
    }
    Ok(())
}

#[driver_test]
pub async fn embedded_literal_required_fields(test: &mut Test) -> Result<()> {
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
            #[shared(label)]
            label: String,
            #[shared(note)]
            note: Option<String>,
        },
        Anonymous {
            #[shared(label)]
            label: String,
            #[shared(note)]
            note: Option<String>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = test.setup_db(models!(Object, Human)).await;
    let human = toasty::create!(Human {}).exec(&mut db).await?;
    let unloaded = toasty::Deferred::<Human>::default();
    test.log().clear();
    let err = assert_err!(
        toasty::create!(Object {
            owner: Owner::Human {
                human: &unloaded,
                label: "note".into()
            }
        })
        .exec(&mut db)
        .await
    );
    assert!(err.is_validation());
    assert!(err.to_string().contains("`id`"));
    let err = assert_err!(
        toasty::create!(Object {
            owner: Owner::Human { human: &human }
        })
        .exec(&mut db)
        .await
    );
    assert!(err.is_validation());
    assert!(err.to_string().contains("`label`"));
    let err = assert_err!(
        toasty::create!(Object {
            owner: Owner::Anonymous {}
        })
        .exec(&mut db)
        .await
    );
    assert!(err.is_validation());
    assert!(err.to_string().contains("`label`"));
    assert!(test.log().is_empty());

    let mut object = toasty::create!(Object {
        owner: Owner::Human {
            human: &human,
            label: "note".into()
        }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(
        object.owner,
        Owner::Human {
            label: "note",
            note: None,
            ..
        }
    );
    test.log().clear();
    let err = assert_err!(
        toasty::update!(object {
            owner: Owner::Human { human: &human }
        })
        .exec(&mut db)
        .await
    );
    assert!(err.is_validation());
    assert!(test.log().is_empty());
    toasty::update!(object {
        owner: Owner::Human {
            human: &human,
            label: "updated".into()
        }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(
        Object::get_by_id(&mut db, object.id).await?.owner,
        Owner::Human {
            label: "updated",
            note: None,
            ..
        }
    );
    let anonymous = Object::create()
        .owner(
            Object::fields()
                .owner()
                .anonymous()
                .create()
                .label("anonymous"),
        )
        .exec(&mut db)
        .await?;
    assert_struct!(
        anonymous.owner,
        Owner::Anonymous {
            label: "anonymous",
            note: None
        }
    );
    Ok(())
}

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

#[driver_test]
pub async fn nullable_embedded_relation_value(test: &mut Test) -> Result<()> {
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
            id: Option<uuid::Uuid>,
            #[belongs_to(key = id)]
            human: toasty::Deferred<Option<Human>>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = test.setup_db(models!(Object, Human)).await;
    let human = toasty::create!(Human {}).exec(&mut db).await?;
    let mut object = toasty::create!(Object {
        owner: Owner::Human { human: &human }
    })
    .exec(&mut db)
    .await?;
    assert_eq!(
        Object::filter(Object::fields().owner().human().eq(&human))
            .exec(&mut db)
            .await?
            .len(),
        1
    );
    object
        .update()
        .owner(Owner::Human {
            id: Some(human.id),
            human: None.into(),
        })
        .exec(&mut db)
        .await?;
    assert_struct!(Object::get_by_id(&mut db, object.id).await?.owner, Owner::Human { id: None, human.is_unloaded(): true });
    Ok(())
}

#[driver_test]
pub async fn embedded_relation_model_values(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Human {
        #[key]
        id: uuid::Uuid,
        name: String,
    }
    #[derive(Debug, toasty::Model)]
    struct Animal {
        #[key]
        id: uuid::Uuid,
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
    test.log().clear();
    assert_err!(
        Object::create()
            .owner(Object::fields().owner().human().create())
            .exec(&mut db)
            .await
    );
    assert!(test.log().is_empty());
    let id = uuid::Uuid::new_v4();
    let alice = toasty::create!(Human { id, name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(Human {
        id: uuid::Uuid::new_v4(),
        name: "Bob"
    })
    .exec(&mut db)
    .await?;
    let rex = toasty::create!(Animal { id }).exec(&mut db).await?;
    let human = &alice;
    let mut object = toasty::create!(Object {
        owner: Owner::Human { human }
    })
    .exec(&mut db)
    .await?;
    toasty::update!(object {
        owner: Owner::Human {
            human: std::convert::identity(human)
        }
    })
    .exec(&mut db)
    .await?;
    let animal = toasty::create!(Object {
        owner: Owner::Animal { animal: &rex }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(object.owner, Owner::Human { id: == alice.id, human.is_unloaded(): true });
    let found = Object::filter(Object::fields().owner().human().eq(&alice))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    let path: toasty::stmt::Path<Object, Human> = Object::fields().owner().human().human().into();
    let expr = toasty::stmt::IntoExpr::into_expr(path);
    let found = Object::filter(expr.ne(&bob)).exec(&mut db).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    let found = Object::filter(Object::fields().owner().human().ne(&bob))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    let found = Object::filter(
        Object::fields()
            .owner()
            .human()
            .matches(|v| v.human().name().eq("Alice")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    {
        let animal = &rex;
        toasty::update!(object {
            owner: Owner::Animal { animal }
        })
        .exec(&mut db)
        .await?;
    }
    assert_struct!(Object::get_by_id(&mut db, object.id).await?.owner, Owner::Animal { id: == rex.id, .. });
    let bob_id = bob.id;
    toasty::update!(object {
        owner: Owner::Human {
            id: uuid::Uuid::nil(),
            human: bob.into(),
        }
    })
    .exec(&mut db)
    .await?;
    assert_struct!(Object::get_by_id(&mut db, object.id).await?.owner, Owner::Human { id: == bob_id, .. });
    assert_struct!(Object::get_by_id(&mut db, animal.id).await?.owner, Owner::Animal { id: == rex.id, .. });
    Ok(())
}

#[driver_test]
pub async fn embedded_struct_relation_model_values(test: &mut Test) -> Result<()> {
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
    #[derive(Debug, toasty::Embed)]
    struct Metadata {
        attribution: Attribution,
    }
    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: uuid::Uuid,
        metadata: Option<Metadata>,
    }
    let mut db = test.setup_db(models!(Post, Author)).await;
    let author = toasty::create!(Author { name: "Alice" })
        .exec(&mut db)
        .await?;
    let author_id = author.id;
    let post = toasty::create!(Post {
        metadata: Some(Metadata {
            attribution: Attribution {
                author_id: uuid::Uuid::nil(),
                author: author.into(),
                note: "draft".into()
            },
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Post { metadata: None })
        .exec(&mut db)
        .await?;
    let author = Author::get_by_id(&mut db, author_id).await?;
    let found = Post::filter(
        Post::fields()
            .metadata()
            .chain(Metadata::fields().attribution().author())
            .eq(&author),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, post.id);
    let found = Post::filter(
        Post::fields()
            .metadata()
            .chain(Metadata::fields().attribution().author().name())
            .eq("Alice"),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].metadata.as_ref().unwrap().attribution.author_id,
        author.id
    );
    let mut borrowed = toasty::create!(Post {
        metadata: Some(Metadata {
            attribution: Attribution {
                author: &author,
                note: "published".into()
            }
        }),
    })
    .exec(&mut db)
    .await?;
    toasty::update!(borrowed {
        metadata: Some(Metadata {
            attribution: Attribution {
                author: &author,
                note: "revised".into()
            }
        }),
    })
    .exec(&mut db)
    .await?;
    assert_eq!(
        Post::get_by_id(&mut db, borrowed.id)
            .await?
            .metadata
            .unwrap()
            .attribution
            .note,
        "revised"
    );
    Ok(())
}

#[driver_test]
pub async fn nested_variant_composite_relation(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = namespace, local = revision)]
    struct Parent {
        namespace: uuid::Uuid,
        revision: i64,
        name: String,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
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
    enum Envelope {
        First { owner: Owner },
        Second { other_owner: Owner },
    }
    #[derive(Debug, toasty::Model)]
    struct Object {
        #[key]
        #[auto]
        id: uuid::Uuid,
        envelope: Envelope,
    }
    let mut db = test.setup_db(models!(Object, Parent)).await;
    let namespace = uuid::Uuid::new_v4();
    let first = toasty::create!(Parent {
        namespace,
        revision: 1,
        name: "first"
    })
    .exec(&mut db)
    .await?;
    let second = toasty::create!(Parent {
        namespace,
        revision: 2,
        name: "second"
    })
    .exec(&mut db)
    .await?;
    let object = toasty::create!(Object {
        envelope: Envelope::First {
            owner: Owner::Parent { parent: &first }
        },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object {
        envelope: Envelope::Second {
            other_owner: Owner::Parent { parent: &first }
        },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object {
        envelope: Envelope::First {
            owner: Owner::Parent { parent: &second }
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
    let path: toasty::stmt::Path<Object, Parent> = Object::fields()
        .envelope()
        .first()
        .owner()
        .parent()
        .parent()
        .into();
    let expr = toasty::stmt::IntoExpr::into_expr(path);
    let found = Object::filter(expr.eq(&first)).exec(&mut db).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    let found = Object::filter(
        Object::fields()
            .envelope()
            .first()
            .owner()
            .parent()
            .matches(|v| v.parent().name().eq("first")),
    )
    .exec(&mut db)
    .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, object.id);
    let variant = Object::fields().envelope().first();
    let third = toasty::create!(Parent {
        namespace: uuid::Uuid::new_v4(),
        revision: 2,
        name: "third"
    })
    .exec(&mut db)
    .await?;
    let third_object = toasty::create!(Object {
        envelope: Envelope::First {
            owner: Owner::Parent { parent: &third }
        }
    })
    .exec(&mut db)
    .await?;
    let found = Object::filter(
        variant
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
        variant
            .owner()
            .parent()
            .matches(|v| v.parent().name().eq("missing")),
    )
    .exec(&mut db)
    .await?;
    assert!(found.is_empty());
    Object::filter_by_id(object.id)
        .update()
        .envelope(
            variant
                .create()
                .owner(variant.owner().parent().create().parent(&second)),
        )
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

    test.log().clear();
    let referenced = toasty::create!(Object {
        owner: Owner::Bot { bot: &bot }
    })
    .exec(&mut db)
    .await?;
    test.log().pop();
    assert!(test.log().is_empty());
    assert_struct!(
        referenced.owner,
        Owner::Bot {
            serial: "B-1000",
            ..
        }
    );
    let found = Object::filter(Object::fields().owner().bot().eq(&bot))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 2);
    referenced.delete().exec(&mut db).await?;

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
