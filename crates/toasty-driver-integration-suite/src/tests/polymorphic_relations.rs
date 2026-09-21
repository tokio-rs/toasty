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
