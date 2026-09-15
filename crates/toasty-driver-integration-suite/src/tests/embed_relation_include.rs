use crate::prelude::*;
use toasty_core::driver::Operation;

#[derive(Debug, toasty::Model)]
struct Person {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
}

#[derive(Debug, toasty::Embed)]
struct Membership {
    id: uuid::Uuid,
    #[belongs_to(key = id)]
    person: Person,
}

#[derive(Debug, toasty::Embed)]
enum EagerOwner {
    Member {
        membership: Membership,
    },
    Person {
        id: uuid::Uuid,
        #[belongs_to(key = id)]
        person: Person,
    },
    Nobody,
}

#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,
    primary: Option<EagerOwner>,
    secondary: Option<EagerOwner>,
}

#[driver_test]
pub async fn eager_nested_embedded_relations(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Document, Person)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(Person { name: "Bob" })
        .exec(&mut db)
        .await?;
    let mut doc = toasty::create!(Document {
        primary: EagerOwner::Member {
            membership: Membership {
                id: alice.id,
                person: alice
            }
        },
        secondary: EagerOwner::Person { person: &bob },
    })
    .exec(&mut db)
    .await?;
    assert_document_owners(&doc);
    let loaded = Document::filter_by_id(doc.id).get(&mut db).await?;
    assert_document_owners(&loaded);
    toasty::create!(Document {
        primary: None,
        secondary: EagerOwner::Nobody
    })
    .exec(&mut db)
    .await?;
    let docs = Document::all().exec(&mut db).await?;
    assert_eq!(docs.len(), 2);
    assert_document_owners(docs.iter().find(|d| d.id == doc.id).unwrap());
    doc.update()
        .primary(EagerOwner::Person {
            id: bob.id,
            person: bob,
        })
        .exec(&mut db)
        .await?;
    assert_struct!(doc.primary, Some(EagerOwner::Person { person.name: "Bob", .. }));
    Ok(())
}

fn assert_document_owners(doc: &Document) {
    assert_struct!(doc.primary, Some(EagerOwner::Member { membership.person.name: "Alice", .. }));
    assert_struct!(doc.secondary, Some(EagerOwner::Person { person.name: "Bob", .. }));
}

#[driver_test]
pub async fn eager_embed_projection(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Card {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: EagerOwner,
    }
    let mut db = t.setup_db(models!(Document, Person, Card)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let doc = toasty::create!(Document {
        primary: EagerOwner::Person { person: &alice },
        secondary: None,
    })
    .exec(&mut db)
    .await?;
    let owner = Document::filter_by_id(doc.id)
        .select(Document::fields().primary())
        .exec(&mut db)
        .await?
        .pop()
        .unwrap();
    assert_struct!(owner, Some(EagerOwner::Person { person.name: "Alice", .. }));
    let (id, owner) = Document::filter_by_id(doc.id)
        .select((Document::fields().id(), Document::fields().primary()))
        .exec(&mut db)
        .await?
        .pop()
        .unwrap();
    assert_eq!(id, doc.id);
    assert_struct!(owner, Some(EagerOwner::Person { person.name: "Alice", .. }));
    let absent = Document::filter_by_id(doc.id)
        .select(Document::fields().secondary())
        .exec(&mut db)
        .await?
        .pop()
        .unwrap();
    assert!(absent.is_none());
    let member = toasty::create!(Card {
        owner: EagerOwner::Member {
            membership: Membership {
                id: alice.id,
                person: alice
            }
        },
    })
    .exec(&mut db)
    .await?;
    let membership = Card::filter_by_id(member.id)
        .select(Card::fields().owner().member().membership())
        .exec(&mut db)
        .await?
        .pop()
        .unwrap();
    assert_eq!(membership.person.name, "Alice");
    Ok(())
}

#[driver_test]
pub async fn eager_embed_key_patch(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Attribution {
        author_id: Option<uuid::Uuid>,
        #[belongs_to(key = author_id)]
        author: Option<Person>,
        label: String,
    }
    #[derive(Debug, toasty::Model)]
    struct Article {
        #[key]
        #[auto]
        id: uuid::Uuid,
        attribution: Attribution,
    }
    let mut db = t.setup_db(models!(Article, Person)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(Person { name: "Bob" })
        .exec(&mut db)
        .await?;
    let mut article = toasty::create!(Article {
        attribution: Attribution {
            author_id: Some(alice.id),
            author: Some(alice),
            label: "by".into()
        },
    })
    .exec(&mut db)
    .await?;
    article
        .update()
        .attribution(toasty::stmt::patch(
            Attribution::fields().author_id(),
            Some(bob.id),
        ))
        .exec(&mut db)
        .await?;
    assert_eq!(article.attribution.author_id, Some(bob.id));
    assert_eq!(article.attribution.author.as_ref().unwrap().name, "Bob");
    assert_eq!(article.attribution.label, "by");
    let stored = Article::filter_by_id(article.id).get(&mut db).await?;
    assert_eq!(stored.attribution.author.as_ref().unwrap().name, "Bob");
    t.log().clear();
    article
        .update()
        .attribution(toasty::stmt::patch(
            Attribution::fields().label(),
            "written by",
        ))
        .exec(&mut db)
        .await?;
    assert_eq!(article.attribution.author.as_ref().unwrap().name, "Bob");
    assert_query_count(t, 1);
    article
        .update()
        .attribution(toasty::stmt::patch(Attribution::fields().author_id(), None))
        .exec(&mut db)
        .await?;
    assert!(article.attribution.author_id.is_none());
    assert!(article.attribution.author.is_none());
    Ok(())
}

#[driver_test]
pub async fn eager_embedded_relations_batch_create(t: &mut Test) -> Result<()> {
    let mut db = t.setup_db(models!(Document, Person)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(Person { name: "Bob" })
        .exec(&mut db)
        .await?;
    let docs = toasty::create!(Document::[
        { primary: EagerOwner::Person { person: &alice }, secondary: None },
        { primary: None, secondary: EagerOwner::Person { person: &bob } },
    ])
    .exec(&mut db)
    .await?;
    assert_struct!(docs[0].primary, Some(EagerOwner::Person { person.name: "Alice", .. }));
    assert!(docs[0].secondary.is_none());
    assert!(docs[1].primary.is_none());
    assert_struct!(docs[1].secondary, Some(EagerOwner::Person { person.name: "Bob", .. }));
    Ok(())
}

#[driver_test]
pub async fn eager_optional_struct_relation(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Member {
        id: Option<uuid::Uuid>,
        #[belongs_to(key = id)]
        person: Option<Person>,
    }

    #[derive(Debug, toasty::Model)]
    struct Group {
        #[key]
        #[auto]
        id: uuid::Uuid,
        member: Option<Member>,
    }

    let mut db = t.setup_db(models!(Group, Person)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let group = toasty::create!(Group {
        member: Member {
            id: Some(alice.id),
            person: None
        },
    })
    .exec(&mut db)
    .await?;
    assert_eq!(group.member.unwrap().person.unwrap().name, "Alice");
    let no_person = toasty::create!(Group {
        member: Member {
            id: None,
            person: None
        }
    })
    .exec(&mut db)
    .await?;
    assert!(no_person.member.unwrap().person.is_none());
    let absent = toasty::create!(Group { member: None })
        .exec(&mut db)
        .await?;
    assert!(absent.member.is_none());
    let groups = Group::all().exec(&mut db).await?;
    assert_eq!(groups.len(), 3);
    assert_eq!(
        groups.into_iter().filter_map(|g| g.member?.person).count(),
        1
    );
    Ok(())
}

#[driver_test]
pub async fn include_nested_relation_path(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Link {
        id: Option<uuid::Uuid>,
        #[belongs_to(key = id)]
        person: toasty::Deferred<Option<Person>>,
        notes: toasty::Deferred<String>,
    }

    #[derive(Debug, toasty::Embed)]
    enum Target {
        Assigned { link: Link },
        Nobody,
    }

    #[derive(Debug, toasty::Model)]
    struct Ticket {
        #[key]
        #[auto]
        id: uuid::Uuid,
        owner: Target,
        backup: toasty::Deferred<Target>,
    }

    let mut db = t.setup_db(models!(Ticket, Person)).await;
    let alice = toasty::create!(Person { name: "Alice" })
        .exec(&mut db)
        .await?;
    let target = |id| Target::Assigned {
        link: Link {
            id,
            person: Default::default(),
            notes: "notes".to_string().into(),
        },
    };
    let ticket = toasty::create!(Ticket {
        owner: target(Some(alice.id)),
        backup: target(None)
    })
    .exec(&mut db)
    .await?;
    let loaded = Ticket::filter_by_id(ticket.id)
        .include(Ticket::fields().owner().assigned().link().person())
        .get(&mut db)
        .await?;
    let Target::Assigned { link } = loaded.owner else {
        panic!("expected assigned")
    };
    assert_eq!(link.person.get().as_ref().unwrap().name, "Alice");
    assert!(link.notes.is_unloaded());
    assert!(loaded.backup.is_unloaded());

    let loaded = Ticket::filter_by_id(ticket.id)
        .include(Ticket::fields().backup())
        .get(&mut db)
        .await?;
    let Target::Assigned { link } = loaded.backup.get() else {
        panic!("expected assigned")
    };
    assert!(link.person.get().is_none());
    assert!(link.notes.is_unloaded());
    Ok(())
}

#[driver_test]
pub async fn include_polymorphic_owner(t: &mut Test) -> Result<()> {
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
        name: String,
    }

    #[derive(Debug, toasty::Model)]
    struct Bot {
        #[key]
        serial: String,
    }

    #[derive(Debug, toasty::Embed)]
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
        owner: Option<Owner>,
    }

    let mut db = t.setup_db(models!(Object, Human, Animal, Bot)).await;
    let id = uuid::Uuid::new_v4();
    let human = toasty::create!(Human { id, name: "Alice" })
        .exec(&mut db)
        .await?;
    let animal = toasty::create!(Animal { id, name: "Cat" })
        .exec(&mut db)
        .await?;
    let bot = toasty::create!(Bot { serial: "robot" })
        .exec(&mut db)
        .await?;
    let a = toasty::create!(Object {
        owner: Owner::Human { human: &human }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object {
        owner: Owner::Animal { animal: &animal }
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Object { owner: None })
        .exec(&mut db)
        .await?;
    toasty::create!(Object {
        owner: Owner::Bot { bot: &bot }
    })
    .exec(&mut db)
    .await?;

    t.log().clear();
    let objects = Object::all()
        .include(Object::fields().owner())
        .exec(&mut db)
        .await?;
    assert_eq!(objects.len(), 4);
    for object in objects {
        match object.owner {
            Some(Owner::Human { human, .. }) => assert_eq!(human.get().name, "Alice"),
            Some(Owner::Animal { animal, .. }) => assert_eq!(animal.get().name, "Cat"),
            Some(Owner::Bot { bot, .. }) => assert_eq!(bot.get().serial, "robot"),
            None => {}
        }
    }
    assert_query_count(t, 4);

    t.log().clear();
    let object = Object::filter_by_id(a.id)
        .include(Object::fields().owner())
        .get(&mut db)
        .await?;
    assert!(matches!(object.owner, Some(Owner::Human { .. })));
    assert_query_count(t, 2);

    t.log().clear();
    let missing = Object::filter_by_id(uuid::Uuid::new_v4())
        .include(Object::fields().owner())
        .exec(&mut db)
        .await?;
    assert!(missing.is_empty());
    assert_query_count(t, 1);
    Ok(())
}

fn assert_query_count(t: &Test, expected: usize) {
    let mut count = 0;
    while !t.log().is_empty() {
        if !matches!(t.log().pop_op(), Operation::Transaction(_)) {
            count += 1;
        }
    }
    assert_eq!(count, expected);
}
