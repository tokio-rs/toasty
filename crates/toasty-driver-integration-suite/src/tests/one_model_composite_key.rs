use crate::prelude::*;

#[driver_test]
pub async fn batch_get_by_key(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Item {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    let mut db = test.setup_db(models!(Item)).await;
    let mut keys = vec![];

    for i in 0..5 {
        let item = Item::create()
            .one(format!("one-{i}"))
            .two(format!("two-{i}"))
            .exec(&mut db)
            .await?;

        keys.push((item.one.clone(), item.two.clone()));
    }

    let items: Vec<_> = Item::filter_by_one_and_two_batch([
        (&keys[0].0, &keys[0].1),
        (&keys[1].0, &keys[1].1),
        (&keys[2].0, &keys[2].1),
    ])
    .exec(&mut db)
    .await?;

    assert_eq!(3, items.len());

    for item in items {
        assert!(keys
            .iter()
            .any(|key| item.one == key.0 && item.two == key.1));
    }
    Ok(())
}
