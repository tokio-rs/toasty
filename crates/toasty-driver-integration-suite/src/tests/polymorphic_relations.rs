use crate::prelude::*;
use toasty_core::driver::Operation;

#[derive(Debug, toasty::Model)]
struct Human {
    #[key]
    id: uuid::Uuid,
    name: String,
    #[has_many]
    objects: toasty::Deferred<Vec<Object>>,
}

#[derive(Debug, toasty::Model)]
struct Animal {
    #[key]
    id: uuid::Uuid,
    name: String,
    #[has_many(pair = owner.animal)]
    objects: toasty::Deferred<Vec<Object>>,
}

#[derive(Debug, toasty::Model)]
struct Bot {
    #[key]
    serial: String,
    name: String,
    #[has_many(pair = owner)]
    objects: toasty::Deferred<Vec<Object>>,
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

#[driver_test]
pub async fn inverse_queries_isolate_shared_keys(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Object, Human, Animal, Bot)).await;
    let id = uuid::Uuid::new_v4();
    let human = toasty::create!(Human { id, name: "Alice" })
        .exec(&mut db)
        .await?;
    let animal = toasty::create!(Animal { id, name: "Cat" })
        .exec(&mut db)
        .await?;
    let bot = toasty::create!(Bot {
        serial: "bot-1",
        name: "Helper"
    })
    .exec(&mut db)
    .await?;
    let a = toasty::create!(Object {
        owner: Owner::Human { human: &human }
    })
    .exec(&mut db)
    .await?;
    let b = toasty::create!(Object {
        owner: Owner::Animal { animal: &animal }
    })
    .exec(&mut db)
    .await?;
    let c = toasty::create!(Object {
        owner: Owner::Bot { bot: &bot }
    })
    .exec(&mut db)
    .await?;
    let objects = human.objects().exec(&mut db).await?;
    assert_struct!(objects, [{ id: == a.id }]);
    let objects = animal.objects().exec(&mut db).await?;
    assert_struct!(objects, [{ id: == b.id }]);
    let objects = bot.objects().exec(&mut db).await?;
    assert_struct!(objects, [{ id: == c.id }]);
    let loaded = Human::filter_by_id(id)
        .include(Human::fields().objects())
        .get(&mut db)
        .await?;
    assert_struct!(loaded.objects.get(), [{ id: == a.id }]);
    Ok(())
}

#[driver_test]
pub async fn include_mixed_owners(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Object, Human, Animal, Bot)).await;
    let id = uuid::Uuid::new_v4();
    let human = toasty::create!(Human { id, name: "Alice" })
        .exec(&mut db)
        .await?;
    let animal = toasty::create!(Animal { id, name: "Cat" })
        .exec(&mut db)
        .await?;
    let bot = toasty::create!(Bot {
        serial: "bot-1",
        name: "Helper"
    })
    .exec(&mut db)
    .await?;
    let mut ids = vec![];
    for _ in 0..2 {
        ids.push(
            toasty::create!(Object {
                owner: Owner::Human { human: &human }
            })
            .exec(&mut db)
            .await?
            .id,
        );
        ids.push(
            toasty::create!(Object {
                owner: Owner::Animal { animal: &animal }
            })
            .exec(&mut db)
            .await?
            .id,
        );
        ids.push(
            toasty::create!(Object {
                owner: Owner::Bot { bot: &bot }
            })
            .exec(&mut db)
            .await?
            .id,
        );
    }
    t.log().clear();
    let objects = Object::filter(Object::fields().id().in_list(ids))
        .include(Object::fields().owner())
        .exec(&mut db)
        .await?;
    assert_eq!(objects.len(), 6);
    assert_eq!(query_count(t), 4);
    let human_ids: Vec<_> = objects
        .iter()
        .filter(|object| matches!(object.owner, Owner::Human { .. }))
        .map(|object| object.id)
        .collect();
    for object in objects {
        match object.owner {
            Owner::Human { human, .. } => assert_eq!(human.get().name, "Alice"),
            Owner::Animal { animal, .. } => assert_eq!(animal.get().name, "Cat"),
            Owner::Bot { bot, .. } => assert_eq!(bot.get().name, "Helper"),
        }
    }
    t.log().clear();
    let objects = Object::filter(Object::fields().id().in_list(human_ids))
        .include(Object::fields().owner())
        .exec(&mut db)
        .await?;
    assert_eq!(objects.len(), 2);
    assert_eq!(query_count(t), 2);
    Ok(())
}

#[driver_test]
pub async fn include_empty_result_skips_owners(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Object, Human, Animal, Bot)).await;
    t.log().clear();
    let objects = Object::filter_by_id(uuid::Uuid::new_v4())
        .include(Object::fields().owner())
        .exec(&mut db)
        .await?;
    assert!(objects.is_empty());
    assert_eq!(query_count(t), 1);
    Ok(())
}

#[driver_test]
pub async fn include_dangling_owner_returns_not_found(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Parent {
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = toasty::create!(Item {
        owner: Owner::Parent { parent: &parent }
    })
    .exec(&mut db)
    .await?;
    parent.delete().exec(&mut db).await?;
    assert_eq!(Item::get_by_id(&mut db, item.id).await?.id, item.id);
    let error = assert_err!(
        Item::filter_by_id(item.id)
            .include(Item::fields().owner())
            .get(&mut db)
            .await
    );
    assert!(error.is_record_not_found(), "{error}");
    Ok(())
}

#[driver_test]
pub async fn create_reassign_and_delete_through_inverse(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Object, Human, Animal, Bot)).await;
    let id = uuid::Uuid::new_v4();
    let human = toasty::create!(Human { id, name: "Alice" })
        .exec(&mut db)
        .await?;
    let animal = toasty::create!(Animal { id, name: "Cat" })
        .exec(&mut db)
        .await?;
    let object = human.objects().create().exec(&mut db).await?;
    assert_struct!(object.owner, Owner::Human { id: == id, .. });
    assert_err!(human.objects().remove(&object).exec(&mut db).await);
    animal.objects().insert(&object).exec(&mut db).await?;
    assert!(human.objects().exec(&mut db).await?.is_empty());
    let stored = Object::get_by_id(&mut db, object.id).await?;
    assert_struct!(stored.owner, Owner::Animal { id: == id, .. });
    let other = human.objects().create().exec(&mut db).await?;
    human.delete().exec(&mut db).await?;
    assert!(
        Object::filter_by_id(other.id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    assert!(
        Object::filter_by_id(object.id)
            .first()
            .exec(&mut db)
            .await?
            .is_some()
    );
    Ok(())
}

#[driver_test]
pub async fn reused_optional_embed_pairs_independently(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many(pair = primary.parent.parent)]
        primary_objects: toasty::Deferred<Vec<Item>>,
        #[has_many(pair = secondary.parent)]
        secondary_objects: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Ownership {
        Parent {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
        Nobody,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        primary: Option<Ownership>,
        secondary: Option<Ownership>,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = parent.primary_objects().create().exec(&mut db).await?;
    assert_struct!(item.primary, Some(Ownership::Parent { id: == parent.id, .. }));
    assert!(parent.secondary_objects().exec(&mut db).await?.is_empty());
    parent
        .secondary_objects()
        .insert(&item)
        .exec(&mut db)
        .await?;
    parent.primary_objects().remove(&item).exec(&mut db).await?;
    let item = Item::filter_by_id(item.id)
        .include(Item::fields().secondary())
        .get(&mut db)
        .await?;
    assert!(item.primary.is_none());
    let Some(Ownership::Parent { parent: loaded, .. }) = item.secondary else {
        panic!()
    };
    assert_eq!(loaded.get().id, parent.id);
    assert_eq!(parent.secondary_objects().exec(&mut db).await?.len(), 1);
    Ok(())
}

#[driver_test]
pub async fn eager_relation_in_nested_struct(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        name: String,
        #[has_many(pair = envelope.owner)]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: Parent,
    }
    #[derive(Debug, toasty::Embed)]
    struct Envelope {
        owner: Owner,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        envelope: Envelope,
    }
    let mut db = t.setup_db(models!(Item, Parent)).await;
    let parent = toasty::create!(Parent { name: "Alice" })
        .exec(&mut db)
        .await?;
    let item = toasty::create!(Item {
        envelope: Envelope {
            owner: Owner {
                id: parent.id,
                parent
            }
        }
    })
    .exec(&mut db)
    .await?;
    assert_eq!(item.envelope.owner.parent.name, "Alice");
    let mut item = Item::get_by_id(&mut db, item.id).await?;
    assert_eq!(item.envelope.owner.parent.name, "Alice");
    assert_eq!(
        item.envelope
            .owner
            .parent
            .items()
            .exec(&mut db)
            .await?
            .len(),
        1
    );
    let parent = toasty::create!(Parent { name: "Bob" })
        .exec(&mut db)
        .await?;
    item.update()
        .envelope(Envelope {
            owner: Owner {
                id: parent.id,
                parent,
            },
        })
        .exec(&mut db)
        .await?;
    assert_eq!(item.envelope.owner.parent.name, "Bob");
    Ok(())
}

#[driver_test(id(ID))]
pub async fn nested_create_through_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: ID,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Parent {
            #[index]
            id: ID,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = Parent::create()
        .items([Item::create(), Item::create()])
        .exec(&mut db)
        .await?;
    let items = parent.items().exec(&mut db).await?;
    assert_eq!(items.len(), 2);
    for item in items {
        assert_struct!(item.owner, Owner::Parent { id: == parent.id, .. });
    }
    Ok(())
}

#[driver_test]
pub async fn has_one_pairs_into_optional_embed(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_one]
        item: toasty::Deferred<Option<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[unique]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Option<Owner>,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let mut parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let first = parent.item().create().exec(&mut db).await?;
    assert_eq!(parent.item().exec(&mut db).await?.unwrap().id, first.id);
    parent.update().item(Item::create()).exec(&mut db).await?;
    let second = parent.item().exec(&mut db).await?.unwrap();
    assert_ne!(second.id, first.id);
    assert!(Item::get_by_id(&mut db, first.id).await?.owner.is_none());
    let parent_value = Parent::get_by_id(&mut db, parent.id).await?;
    let third = toasty::create!(Item {
        owner: Some(Owner {
            id: parent.id,
            parent: parent_value.into()
        })
    })
    .exec(&mut db)
    .await?;
    assert_eq!(parent.item().exec(&mut db).await?.unwrap().id, third.id);
    assert!(Item::get_by_id(&mut db, second.id).await?.owner.is_none());
    parent.delete().exec(&mut db).await?;
    assert!(Item::get_by_id(&mut db, third.id).await?.owner.is_none());
    Ok(())
}

#[driver_test]
pub async fn inverse_composite_key_in_nested_variant(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = id, local = revision)]
    struct Parent {
        id: uuid::Uuid,
        revision: i64,
        #[has_many(pair = owner.parent.key.parent)]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    #[index(id, revision)]
    struct Key {
        id: uuid::Uuid,
        revision: i64,
        #[belongs_to(key = [id, revision], references = [id, revision])]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Nobody,
        Parent { key: Key },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let id = uuid::Uuid::new_v4();
    let first = toasty::create!(Parent { id, revision: 1 })
        .exec(&mut db)
        .await?;
    let second = toasty::create!(Parent { id, revision: 2 })
        .exec(&mut db)
        .await?;
    let item = first.items().create().exec(&mut db).await?;
    assert!(second.items().exec(&mut db).await?.is_empty());
    let loaded = Item::filter_by_id(item.id)
        .include(Item::fields().owner())
        .get(&mut db)
        .await?;
    let Owner::Parent { key } = loaded.owner else {
        panic!()
    };
    assert_eq!(key.parent.get().revision, 1);
    second.items().insert(&item).exec(&mut db).await?;
    assert!(first.items().exec(&mut db).await?.is_empty());
    assert_eq!(second.items().exec(&mut db).await?.len(), 1);
    Ok(())
}

fn query_count(t: &Test) -> usize {
    let mut count = 0;
    while !t.log().is_empty() {
        if !matches!(t.log().pop_op(), Operation::Transaction(_)) {
            count += 1;
        }
    }
    count
}

#[driver_test]
pub async fn reassign_preserves_struct_siblings(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
        label: String,
    }
    #[derive(Debug, toasty::Embed)]
    struct Envelope {
        owner: Owner,
        note: Option<String>,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        envelope: Envelope,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let first = toasty::create!(Parent {}).exec(&mut db).await?;
    let second = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = toasty::create!(Item {
        envelope: Envelope {
            owner: Owner {
                id: first.id,
                parent: toasty::Deferred::default(),
                label: "keep label".to_string()
            },
            note: Some("keep note".to_string()),
        },
    })
    .exec(&mut db)
    .await?;
    second.items().insert(&item).exec(&mut db).await?;
    let stored = Item::get_by_id(&mut db, item.id).await?;
    assert_eq!(stored.envelope.note.as_deref(), Some("keep note"));
    assert_eq!(stored.envelope.owner.label, "keep label");
    assert_eq!(stored.envelope.owner.id, second.id);
    Ok(())
}

#[driver_test]
pub async fn clear_nested_optional_owner_preserves_child(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[index]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Envelope {
        owner: Option<Owner>,
        note: String,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        envelope: Envelope,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = toasty::create!(Item {
        envelope: Envelope {
            owner: Some(Owner {
                id: parent.id,
                parent: toasty::Deferred::default()
            }),
            note: "keep".to_string()
        },
    })
    .exec(&mut db)
    .await?;
    parent.items().remove(&item).exec(&mut db).await?;
    let stored = Item::get_by_id(&mut db, item.id).await?;
    assert!(stored.envelope.owner.is_none());
    assert_eq!(stored.envelope.note, "keep");
    parent.items().insert(&item).exec(&mut db).await?;
    parent.delete().exec(&mut db).await?;
    let stored = Item::get_by_id(&mut db, item.id).await?;
    assert!(stored.envelope.owner.is_none());
    assert_eq!(stored.envelope.note, "keep");
    Ok(())
}

#[driver_test]
pub async fn same_owner_update_preserves_required_inverse(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_one]
        item: toasty::Deferred<Item>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[unique]
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Parent>,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Option<Owner>,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = Parent::create().item(Item::create()).exec(&mut db).await?;
    let mut item = parent.item().exec(&mut db).await?;
    item.update()
        .owner(Some(Owner {
            id: parent.id,
            parent: toasty::Deferred::default(),
        }))
        .exec(&mut db)
        .await?;
    assert_eq!(Parent::get_by_id(&mut db, parent.id).await?.id, parent.id);
    assert_eq!(parent.item().exec(&mut db).await?.id, item.id);
    let parent_id = parent.id;
    item.update()
        .owner(Some(Owner {
            id: parent_id,
            parent: parent.into(),
        }))
        .exec(&mut db)
        .await?;
    assert_eq!(Parent::get_by_id(&mut db, parent_id).await?.id, parent_id);
    item.update().owner(None).exec(&mut db).await?;
    assert!(
        Parent::filter_by_id(parent_id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    assert!(Item::get_by_id(&mut db, item.id).await?.owner.is_none());
    Ok(())
}

#[driver_test]
pub async fn clear_optional_relation_preserves_required_embed(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    struct Owner {
        #[index]
        id: Option<uuid::Uuid>,
        #[belongs_to(key = id)]
        parent: toasty::Deferred<Option<Parent>>,
        note: String,
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = toasty::create!(Item {
        owner: Owner {
            id: Some(parent.id),
            parent: toasty::Deferred::default(),
            note: "keep".to_string()
        },
    })
    .exec(&mut db)
    .await?;
    parent.items().remove(&item).exec(&mut db).await?;
    let stored = Item::get_by_id(&mut db, item.id).await?;
    assert!(stored.owner.id.is_none());
    assert_eq!(stored.owner.note, "keep");
    assert!(parent.items().exec(&mut db).await?.is_empty());
    Ok(())
}

#[driver_test(requires(sql))]
pub async fn via_include_embedded_pair_is_rejected(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
        #[has_many(via = items)]
        via_items: toasty::Deferred<Vec<Item>>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Parent {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    t.log().clear();
    let error = assert_err!(
        Parent::all()
            .include(Parent::fields().via_items())
            .exec(&mut db)
            .await
    );
    assert!(error.is_unsupported_feature(), "{error}");
    assert!(t.log().is_empty());
    Ok(())
}

#[driver_test]
pub async fn one_embed_pairs_on_different_hosts(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_many]
        items: toasty::Deferred<Vec<Item>>,
        #[has_many]
        widgets: toasty::Deferred<Vec<Widget>>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Parent {
            #[index]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: Parent,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Owner,
    }
    #[derive(Debug, toasty::Embed)]
    struct Envelope {
        owner: toasty::Deferred<Owner>,
    }
    #[derive(Debug, toasty::Model)]
    struct Widget {
        #[key]
        #[auto]
        id: uuid::Uuid,
        envelope: Envelope,
    }
    let mut db = t.setup_db(models!(Parent, Item, Widget)).await;
    let parent = toasty::create!(Parent {}).exec(&mut db).await?;
    let item = parent.items().create().exec(&mut db).await?;
    let widget = parent.widgets().create().exec(&mut db).await?;
    assert_struct!(item.owner, Owner::Parent { parent.id: == parent.id, .. });
    let loaded = Widget::filter_by_id(widget.id)
        .include(Widget::fields().envelope())
        .get(&mut db)
        .await?;
    assert_struct!(loaded.envelope.owner.get(), Owner::Parent { parent.id: == parent.id, .. });
    assert_eq!(parent.items().exec(&mut db).await?.len(), 1);
    assert_eq!(parent.widgets().exec(&mut db).await?.len(), 1);
    parent.delete().exec(&mut db).await?;
    assert!(
        Item::filter_by_id(item.id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    assert!(
        Widget::filter_by_id(widget.id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    Ok(())
}

#[driver_test]
pub async fn deleting_child_cascades_required_inverse_once(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,
        #[has_one]
        item: toasty::Deferred<Item>,
    }
    #[derive(Debug, toasty::Embed)]
    enum Owner {
        Parent {
            #[unique]
            id: uuid::Uuid,
            #[belongs_to(key = id)]
            parent: toasty::Deferred<Parent>,
        },
    }
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Option<Owner>,
    }
    let mut db = t.setup_db(models!(Parent, Item)).await;
    let parent = Parent::create().item(Item::create()).exec(&mut db).await?;
    let other = Parent::create().item(Item::create()).exec(&mut db).await?;
    let mut detached = other.item().exec(&mut db).await?;
    detached
        .update()
        .owner(Some(Owner::Parent {
            id: other.id,
            parent: toasty::Deferred::default(),
        }))
        .exec(&mut db)
        .await?;
    assert_eq!(Parent::get_by_id(&mut db, other.id).await?.id, other.id);
    detached.update().owner(None).exec(&mut db).await?;
    assert!(
        Parent::filter_by_id(other.id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    assert!(Item::get_by_id(&mut db, detached.id).await?.owner.is_none());
    let item = parent.item().exec(&mut db).await?;
    let item_id = item.id;
    item.delete().exec(&mut db).await?;
    assert!(
        Parent::filter_by_id(parent.id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    assert!(
        Item::filter_by_id(item_id)
            .first()
            .exec(&mut db)
            .await?
            .is_none()
    );
    Ok(())
}
