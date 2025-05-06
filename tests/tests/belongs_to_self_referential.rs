use tests::*;
use toasty::stmt::Id;

use std::collections::HashMap;

async fn crud_person_self_referential(s: impl Setup) {
    #[derive(Debug, toasty::Model)]
    struct Person {
        #[key]
        #[auto]
        id: Id<Self>,

        name: String,

        #[index]
        parent_id: Option<Id<Person>>,

        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::BelongsTo<Option<Person>>,

        #[has_many(pair = parent)]
        children: toasty::HasMany<Person>,
    }

    let db = s.setup(models!(Person)).await;

    let p1 = Person::create().name("person 1").exec(&db).await.unwrap();

    assert_none!(p1.parent_id);

    // Associate P2 with P1 on creation by value.
    let p2 = Person::create()
        .name("person 2")
        .parent(&p1)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(p2.parent_id, Some(p1.id.clone()));

    // Associate P3 with P1 by ID.
    let p3 = Person::create()
        .name("person 3")
        .parent_id(&p1.id)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(p3.parent_id, Some(p1.id.clone()));

    let assert = |children: &[Person]| {
        assert_eq!(children.len(), 2);

        let children: HashMap<_, _> = children.iter().map(|p| (p.id.clone(), p)).collect();
        assert_eq!(children.len(), 2);

        for (id, child) in &children {
            if id == &p2.id {
                assert_eq!(child.name, "person 2");
            } else if id == &p3.id {
                assert_eq!(child.name, "person 3");
            } else {
                panic!("Unexpected child ID: {}", id);
            }
        }
    };

    // Load children from parent
    let children: Vec<_> = p1.children().collect(&db).await.unwrap();
    assert(&children);

    // Try preloading this time
    let p1 = Person::filter_by_id(&p1.id)
        .include(Person::FIELDS.children)
        .get(&db)
        .await
        .unwrap();

    assert(p1.children.get());
}

tests!(crud_person_self_referential,);
