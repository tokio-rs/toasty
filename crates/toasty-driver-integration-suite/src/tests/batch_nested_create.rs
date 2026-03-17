use crate::prelude::*;

/// Use an array of create builders to create multiple nested HasMany records
/// in a single parent create statement.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn batch_as_nested_has_many_create(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    // Pass an array of create builders — arrays and slices implement
    // `IntoExpr<List<Model>>` so they work as nested HasMany values.
    let user = User::create()
        .name("Ann Chovey")
        .todos([
            Todo::create().title("Make pizza"),
            Todo::create().title("Sleep"),
        ])
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Ann Chovey");

    // Verify both todos were created and linked
    let todos: Vec<_> = user.todos().exec(&mut db).await?;
    assert_eq_unordered!(todos.iter().map(|t| &t.title[..]), ["Make pizza", "Sleep"]);

    Ok(())
}
