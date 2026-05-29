//! `.select(...)` projection through a `BelongsTo` relation field.
//!
//! Projects the related-model side of the relation directly: a query rooted
//! at the source model returns one related-model record per source row.

use crate::prelude::*;

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to)
)]
pub async fn select_belongs_to_basic(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(Todo {
        title: "Hello",
        user: alice
    })
    .exec(&mut db)
    .await?;

    let users: Vec<User> = Todo::all()
        .select(Todo::fields().user())
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Alice");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to)
)]
pub async fn select_belongs_to_with_filter(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let bob = toasty::create!(User { name: "Bob" }).exec(&mut db).await?;
    toasty::create!(Todo::[
        { title: "Alpha", user: alice },
        { title: "Beta",  user: bob },
    ])
    .exec(&mut db)
    .await?;

    let users: Vec<User> = Todo::filter(Todo::fields().title().eq("Beta"))
        .select(Todo::fields().user())
        .exec(&mut db)
        .await?;

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, "Bob");
    Ok(())
}

#[driver_test(
    id(ID),
    requires(scan),
    scenario(crate::scenarios::has_many_belongs_to)
)]
pub async fn select_belongs_to_first(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let alice = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    toasty::create!(Todo {
        title: "Hello",
        user: alice
    })
    .exec(&mut db)
    .await?;

    let user: Option<User> = Todo::filter(Todo::fields().title().eq("Hello"))
        .select(Todo::fields().user())
        .first()
        .exec(&mut db)
        .await?;

    assert_eq!(user.map(|u| u.name).as_deref(), Some("Alice"));
    Ok(())
}
