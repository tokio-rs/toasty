use crate::prelude::*;

#[driver_test]
pub async fn filter_composite_key_in_list(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    for i in 0..5 {
        Foo::create()
            .one(format!("foo-{i}"))
            .two(format!("bar-{i}"))
            .exec(&mut db)
            .await?;
    }

    // Use the free function form with a tuple of field paths
    let foos: Vec<_> = Foo::filter(toasty::stmt::in_list(
        (Foo::fields().one(), Foo::fields().two()),
        [("foo-1", "bar-1"), ("foo-3", "bar-3")],
    ))
    .all(&mut db)
    .await?;

    assert_eq!(foos.len(), 2);

    #[allow(clippy::disallowed_names)]
    for foo in &foos {
        assert!(
            (foo.one == "foo-1" && foo.two == "bar-1")
                || (foo.one == "foo-3" && foo.two == "bar-3")
        );
    }

    Ok(())
}

#[driver_test]
pub async fn filter_composite_key_in_list_empty(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Foo {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let mut db = test.setup_db(models!(Foo)).await;

    Foo::create().one("a").two("b").exec(&mut db).await?;

    let empty: Vec<(String, String)> = vec![];
    let foos: Vec<_> = Foo::filter(toasty::stmt::in_list(
        (Foo::fields().one(), Foo::fields().two()),
        empty,
    ))
    .all(&mut db)
    .await?;

    assert_eq!(foos.len(), 0);

    Ok(())
}
