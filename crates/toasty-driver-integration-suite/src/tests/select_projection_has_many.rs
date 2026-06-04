//! `.select(...)` projection through a `HasMany` relation field.
//!
//! Field handles for `HasMany` relations return
//! `<Target as Relation>::ManyField<__Origin>` (the macro-generated
//! `*FieldList` struct).  An `IntoExpr<List<TargetModel>>` impl on that
//! struct lets the field handle flow through `.select(...)` the same way
//! `BelongsTo`/`HasOne` handles do; each parent row projects to a list of
//! related rows, and the executor decodes the result as `Vec<Vec<Target>>`.

use crate::prelude::*;

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to), requires(sql))]
pub async fn select_has_many_basic(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User {
        name: "Alice",
        todos: [Todo::create().title("alpha"), Todo::create().title("beta"),],
    })
    .exec(&mut db)
    .await?;

    let todos_per_user: Vec<Vec<Todo>> = User::all()
        .select(User::fields().todos())
        .exec(&mut db)
        .await?;

    assert_eq!(todos_per_user.len(), 1);
    let mut titles: Vec<String> = todos_per_user[0].iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to), requires(sql))]
pub async fn select_has_many_with_filter(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User {
        name: "Alice",
        todos: [Todo::create().title("alpha")],
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "Bob",
        todos: [
            Todo::create().title("beta one"),
            Todo::create().title("beta two"),
        ],
    })
    .exec(&mut db)
    .await?;

    let todos_per_user: Vec<Vec<Todo>> = User::filter(User::fields().name().eq("Bob"))
        .select(User::fields().todos())
        .exec(&mut db)
        .await?;

    assert_eq!(todos_per_user.len(), 1);
    let mut titles: Vec<String> = todos_per_user[0].iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["beta one".to_string(), "beta two".to_string()]);

    Ok(())
}

#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to), requires(sql))]
pub async fn select_has_many_first(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    toasty::create!(User {
        name: "Alice",
        todos: [Todo::create().title("alpha"), Todo::create().title("beta"),],
    })
    .exec(&mut db)
    .await?;

    let todos: Option<Vec<Todo>> = User::filter(User::fields().name().eq("Alice"))
        .select(User::fields().todos())
        .first()
        .exec(&mut db)
        .await?;

    let todos = todos.expect("first() returned None for a matching user");
    let mut titles: Vec<String> = todos.iter().map(|p| p.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["alpha".to_string(), "beta".to_string()]);

    Ok(())
}
