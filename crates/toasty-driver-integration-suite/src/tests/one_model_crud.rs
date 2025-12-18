use crate::prelude::*;

#[driver_test]
pub(crate) async fn crud_no_fields(t: &mut Test) {
    const MORE: i32 = 10;

    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        #[auto]
        id: ID,
    }

    let db = t.setup_db(models!(Foo)).await;

    let created = Foo::create().exec(&db).await.unwrap();

    // Find Foo
    let read = Foo::filter_by_id(&created.id)
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();

    assert_eq!(1, read.len());
    assert_eq!(created.id, read[0].id);

    // Generate a few instances, IDs should be different

    let mut ids = vec![];

    for _ in 0..MORE {
        let item = Foo::create().exec(&db).await.unwrap();
        assert_ne!(item.id, created.id);
        ids.push(item.id);
    }

    assert_unique!(ids);

    for id in &ids {
        let read = Foo::filter_by_id(id)
            .all(&db)
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
            .unwrap();

        assert_eq!(1, read.len());
        assert_eq!(*id, read[0].id);
    }

    // Randomize the IDs
    ids.shuffle();

    // Delete the IDs
    for i in 0..MORE {
        let id = ids.pop().unwrap();

        if i.is_even() {
            // Delete by object
            let val = Foo::get_by_id(&db, &id).await.unwrap();
            val.delete(&db).await.unwrap();
        } else {
            // Delete by ID
            Foo::filter_by_id(&id).delete(&db).await.unwrap();
        }

        // Assert deleted
        assert_err!(Foo::get_by_id(&db, id).await);

        // Assert other foos remain
        for id in &ids {
            let item = Foo::get_by_id(&db, id).await.unwrap();
            assert_eq!(*id, item.id);
        }
    }
}
