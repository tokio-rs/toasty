use crate::prelude::*;

#[driver_test]
pub async fn crud(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        one: String,

        #[key]
        two: String,

        val: String,
    }

    let db = test.setup_db(models!(Foo)).await;

    // === Create ===
    #[allow(clippy::disallowed_names)]
    let foo1 = Foo::create()
        .one("a")
        .two("1")
        .val("first")
        .exec(&db)
        .await
        .unwrap();

    assert_eq!(foo1.one, "a");
    assert_eq!(foo1.two, "1");
    assert_eq!(foo1.val, "first");

    let _foo2 = Foo::create()
        .one("a")
        .two("2")
        .val("second")
        .exec(&db)
        .await
        .unwrap();

    let _foo3 = Foo::create()
        .one("b")
        .two("1")
        .val("third")
        .exec(&db)
        .await
        .unwrap();

    // === Read (get single by composite key) ===
    let loaded = Foo::get_by_one_and_two(&db, "a", "1").await.unwrap();
    assert_eq!(loaded.one, "a");
    assert_eq!(loaded.two, "1");
    assert_eq!(loaded.val, "first");

    let loaded = Foo::get_by_one_and_two(&db, "a", "2").await.unwrap();
    assert_eq!(loaded.val, "second");

    let loaded = Foo::get_by_one_and_two(&db, "b", "1").await.unwrap();
    assert_eq!(loaded.val, "third");

    // Non-existent composite key returns an error
    assert_err!(Foo::get_by_one_and_two(&db, "z", "9").await);

    // === Read (filter) ===
    let results: Vec<_> = Foo::filter_by_one_and_two("a", "1")
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(1, results.len());
    assert_eq!(results[0].val, "first");

    // Non-existent key returns empty results
    let results: Vec<_> = Foo::filter_by_one_and_two("z", "9")
        .all(&db)
        .await
        .unwrap()
        .collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(0, results.len());

    // === Update by object ===
    #[allow(clippy::disallowed_names)]
    let mut foo1 = Foo::get_by_one_and_two(&db, "a", "1").await.unwrap();
    foo1.update().val("updated-first").exec(&db).await.unwrap();
    assert_eq!(foo1.val, "updated-first");

    // Verify update persisted
    let reloaded = Foo::get_by_one_and_two(&db, "a", "1").await.unwrap();
    assert_eq!(reloaded.val, "updated-first");

    // Other records are unaffected
    let reloaded = Foo::get_by_one_and_two(&db, "a", "2").await.unwrap();
    assert_eq!(reloaded.val, "second");

    // === Update by filter ===
    Foo::filter_by_one_and_two("a", "2")
        .update()
        .val("updated-second")
        .exec(&db)
        .await
        .unwrap();

    let reloaded = Foo::get_by_one_and_two(&db, "a", "2").await.unwrap();
    assert_eq!(reloaded.val, "updated-second");

    // Other records still unaffected
    let reloaded = Foo::get_by_one_and_two(&db, "b", "1").await.unwrap();
    assert_eq!(reloaded.val, "third");

    // === Delete by object ===
    let to_delete = Foo::get_by_one_and_two(&db, "a", "1").await.unwrap();
    to_delete.delete(&db).await.unwrap();

    // Verify it's gone
    assert_err!(Foo::get_by_one_and_two(&db, "a", "1").await);

    // Other records still exist
    assert_eq!(
        Foo::get_by_one_and_two(&db, "a", "2").await.unwrap().val,
        "updated-second"
    );
    assert_eq!(
        Foo::get_by_one_and_two(&db, "b", "1").await.unwrap().val,
        "third"
    );

    // === Delete by filter ===
    Foo::filter_by_one_and_two("a", "2")
        .delete(&db)
        .await
        .unwrap();

    // Verify it's gone
    assert_err!(Foo::get_by_one_and_two(&db, "a", "2").await);

    // Last record still exists
    assert_eq!(
        Foo::get_by_one_and_two(&db, "b", "1").await.unwrap().val,
        "third"
    );
}

#[driver_test]
pub async fn batch_get_by_key(test: &mut Test) {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let db = test.setup_db(models!(Foo)).await;
    let mut keys = vec![];

    for i in 0..5 {
        #[allow(clippy::disallowed_names)]
        let foo = Foo::create()
            .one(format!("foo-{i}"))
            .two(format!("bar-{i}"))
            .exec(&db)
            .await
            .unwrap();

        keys.push((foo.one.clone(), foo.two.clone()));
    }

    let foos: Vec<_> = Foo::filter_by_one_and_two_batch([
        (&keys[0].0, &keys[0].1),
        (&keys[1].0, &keys[1].1),
        (&keys[2].0, &keys[2].1),
    ])
    .collect(&db)
    .await
    .unwrap();

    assert_eq!(3, foos.len());

    #[allow(clippy::disallowed_names)]
    for foo in foos {
        assert!(keys.iter().any(|key| foo.one == key.0 && foo.two == key.1));
    }
}
