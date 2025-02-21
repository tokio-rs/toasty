use tests_client::*;

use std::collections::HashSet;

async fn crud_person_self_referential(s: impl Setup) {
    schema!(
        "
        model Person {
            #[key]
            #[auto]
            id: Id,

            name: String,

            #[index]
            parent_id: Option<Id<Person>>,

            #[relation(key = parent_id, references = id)]
            parent: Option<Person>,

            children: [Person],
        }
        "
    );

    let db = s.setup(db::load_schema()).await;

    let p1 = db::Person::create()
        .name("person 1")
        .exec(&db)
        .await
        .unwrap();

    assert_none!(p1.parent_id);

    // Associate P2 with P1 on creation by value.
    let p2 = db::Person::create()
        .name("person 2")
        .parent(&p1)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(p2.parent_id, Some(p1.id.clone()));

    // Associate P3 with P1 by ID.
    let p3 = db::Person::create()
        .name("person 3")
        .parent_id(&p1.id)
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(p3.parent_id, Some(p1.id.clone()));

    let assert = |children: &[db::Person]| {
        assert_eq!(children.len(), 2);

        let ids: HashSet<_> = children.iter().map(|p| p.id.clone()).collect();
        assert_eq!(ids.len(), 2);

        for id in &ids {
            assert!(id == &p2.id || id == &p3.id);
        }
    };

    // Load children from parent
    let children: Vec<_> = p1.children().collect(&db).await.unwrap();
    assert(&children);

    // Try preloading this time
    let p1 = db::Person::filter_by_id(&p1.id)
        .include(db::Person::CHILDREN)
        .get(&db)
        .await
        .unwrap();

    assert(p1.children.get());
}

tests!(crud_person_self_referential,);
