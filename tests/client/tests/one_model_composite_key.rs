use tests_client::*;

async fn batch_get_by_key(s: impl Setup) {
    #[derive(Debug)]
    #[toasty::model]
    struct Foo {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let db = s.setup(models!(Foo)).await;
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

    let foos: Vec<_> = Foo::filter_by_one_and_two_batch(&[
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

tests!(batch_get_by_key,);
