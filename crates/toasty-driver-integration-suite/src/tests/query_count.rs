use crate::prelude::*;

#[driver_test(requires(sql), scenario(crate::scenarios::two_models))]
pub async fn count_empty_table(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let count = User::all().count().exec(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::two_models))]
pub async fn count_after_inserts(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User::[
        { name: "a" },
        { name: "b" },
        { name: "c" },
    ])
    .exec(&mut db)
    .await?;

    let count = User::all().count().exec(&mut db).await?;
    assert_eq!(count, 3);

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::two_models))]
pub async fn count_with_filter(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User::[
        { name: "a" },
        { name: "a" },
        { name: "b" },
    ])
    .exec(&mut db)
    .await?;

    let count = User::filter_by_name("a").count().exec(&mut db).await?;
    assert_eq!(count, 2);

    let count = User::filter_by_name("b").count().exec(&mut db).await?;
    assert_eq!(count, 1);

    let count = User::filter_by_name("c").count().exec(&mut db).await?;
    assert_eq!(count, 0);

    Ok(())
}

#[driver_test(requires(sql), scenario(crate::scenarios::two_models))]
pub async fn count_with_filter_that_folds_to_false(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User::[
        { name: "a" },
        { name: "b" },
    ])
    .exec(&mut db)
    .await?;

    // `name == "a" AND name == "b"` is folded to `false` before it reaches the
    // driver. An aggregate without `GROUP BY` still yields one row, so the
    // count is `0` rather than an empty result.
    let name = User::fields().name();
    let count = User::filter(name.clone().eq("a").and(name.eq("b")))
        .count()
        .exec(&mut db)
        .await?;
    assert_eq!(count, 0);

    Ok(())
}
