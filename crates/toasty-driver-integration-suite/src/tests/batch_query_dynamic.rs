use crate::prelude::*;

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_name))]
pub async fn batch_vec_of_queries(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    User::create().name("Carol").exec(&mut db).await?;

    let results: Vec<Vec<User>> = toasty::batch(vec![
        User::filter_by_name("Alice"),
        User::filter_by_name("Bob"),
        User::filter_by_name("Carol"),
    ])
    .exec(&mut db)
    .await?;

    assert_struct!(results, [[_ { name: "Alice" }], [_ { name: "Bob" }], [_ { name: "Carol" }]]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_name))]
pub async fn batch_array_of_queries(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;

    let results: Vec<Vec<User>> =
        toasty::batch([User::filter_by_name("Alice"), User::filter_by_name("Bob")])
            .exec(&mut db)
            .await?;

    assert_struct!(results, [[_ { name: "Alice" }], [_ { name: "Bob" }]]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_name))]
pub async fn batch_vec_some_empty(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    User::create().name("Alice").exec(&mut db).await?;

    let results: Vec<Vec<User>> = toasty::batch(vec![
        User::filter_by_name("Alice"),
        User::filter_by_name("nobody"),
    ])
    .exec(&mut db)
    .await?;

    assert_struct!(results, [[_ { name: "Alice" }], []]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::two_models))]
pub async fn batch_nested_tuple_with_vec(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    User::create().name("Alice").exec(&mut db).await?;
    User::create().name("Bob").exec(&mut db).await?;
    Post::create().title("Hello").exec(&mut db).await?;
    Post::create().title("World").exec(&mut db).await?;

    // Tuple of (Vec-batch of user queries, Vec-batch of post queries)
    let (users, posts): (Vec<Vec<User>>, Vec<Vec<Post>>) = toasty::batch((
        vec![User::filter_by_name("Alice"), User::filter_by_name("Bob")],
        vec![
            Post::filter_by_title("Hello"),
            Post::filter_by_title("World"),
        ],
    ))
    .exec(&mut db)
    .await?;

    assert_struct!(users, [[_ { name: "Alice" }], [_ { name: "Bob" }]]);
    assert_struct!(posts, [[_ { title: "Hello" }], [_ { title: "World" }]]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_name))]
pub async fn batch_vec_all_empty(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let results: Vec<Vec<User>> = toasty::batch(vec![
        User::filter_by_name("nobody"),
        User::filter_by_name("ghost"),
    ])
    .exec(&mut db)
    .await?;

    assert_struct!(results, [[], []]);

    Ok(())
}
