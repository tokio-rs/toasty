//! Mutation methods (`insert`, `remove`) on a multi-step relation traversal
//! are not supported. The check is at exec time — the type system allows the
//! call but the executor returns an error.
//!
//! Before unifying `Many`, `One`, and `OptionOne` into a single per-model
//! `Query<T>`, calling `.insert()` on a `via` relation was a compile error
//! because `Many<Via>` had no such method. The unified design moves that
//! rejection to runtime so the surface area stays small.

use crate::prelude::*;

#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::user_org_project_todo)
)]
pub async fn insert_on_multi_step_traversal_errors(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let org = toasty::create!(Organization {
        name: "Acme",
        user: &user
    })
    .exec(&mut db)
    .await?;
    let project = toasty::create!(Project {
        name: "p",
        organization: &org
    })
    .exec(&mut db)
    .await?;
    let _todo = toasty::create!(Todo {
        title: "t",
        project: &project
    })
    .exec(&mut db)
    .await?;

    // `user.organizations()` is a single-step relation; chaining `.projects()`
    // produces a query whose underlying association is two steps. `.insert`
    // requires a single-step scope, so the executor rejects it.
    let chained = user.organizations().projects();
    let err = assert_err!(chained.insert(&project).exec(&mut db).await);
    assert!(err.to_string().to_lowercase().contains("multi-step"));

    let chained = user.organizations().projects();
    let err = assert_err!(chained.remove(&project).exec(&mut db).await);
    assert!(err.to_string().to_lowercase().contains("multi-step"));

    Ok(())
}
