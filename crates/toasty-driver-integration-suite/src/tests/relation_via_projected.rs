use crate::prelude::*;

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_projected_via))]
pub async fn has_many_projected_terminal_exec_include_and_select(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let user = toasty::create!(User { name: "Alice" })
        .exec(&mut db)
        .await?;
    let todos = toasty::create!(Todo::[
        { title: "one", user: &user },
        { title: "two", user: &user },
    ])
    .exec(&mut db)
    .await?;

    toasty::create!(Tag::[
        { name: "rust", todo: &todos[0] },
        { name: "orm", todo: &todos[0] },
        { name: "rust", todo: &todos[1] },
    ])
    .exec(&mut db)
    .await?;

    let names = user.tag_names().exec(&mut db).await?;
    let mut names: Vec<_> = names.iter().map(|name| &name[..]).collect();
    names.sort();
    assert_eq!(names, ["orm", "rust", "rust"]);

    let filtered = user
        .tag_names()
        .filter(Tag::fields().name().eq("orm"))
        .exec(&mut db)
        .await?;
    assert_eq!(filtered, ["orm"]);

    let loaded = User::filter_by_id(user.id)
        .include(User::fields().tag_names())
        .get(&mut db)
        .await?;
    let mut loaded_names: Vec<_> = loaded
        .tag_names
        .get()
        .iter()
        .map(|name| &name[..])
        .collect();
    loaded_names.sort();
    assert_eq!(loaded_names, ["orm", "rust", "rust"]);
    let mut eager_names: Vec<_> = loaded
        .eager_tag_names
        .iter()
        .map(|name| &name[..])
        .collect();
    eager_names.sort();
    assert_eq!(eager_names, ["orm", "rust", "rust"]);

    let selected: Vec<Vec<String>> = User::filter_by_id(user.id)
        .select(User::fields().tag_names())
        .exec(&mut db)
        .await?;
    assert_eq!(selected.len(), 1);
    let mut selected_names: Vec<_> = selected[0].iter().map(|name| &name[..]).collect();
    selected_names.sort();
    assert_eq!(selected_names, ["orm", "rust", "rust"]);

    Ok(())
}

#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_projected_via))]
pub async fn has_one_projected_terminal_exec_and_nullable_include(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;

    let users = toasty::create!(User::[
        { name: "Alice" },
        { name: "Bob" },
        { name: "Carol" },
    ])
    .exec(&mut db)
    .await?;
    let (alice, bob, _carol) = (&users[0], &users[1], &users[2]);

    toasty::create!(Profile::[
        {
            display_name: "Alice A.",
            nickname: Some("ally".to_string()),
            user: alice,
        },
        {
            display_name: "Bob B.",
            nickname: None,
            user: bob,
        },
    ])
    .exec(&mut db)
    .await?;

    let display_name = alice.display_name().exec(&mut db).await?;
    assert_eq!(display_name, "Alice A.");

    let loaded = User::all()
        .include(User::fields().nickname())
        .exec(&mut db)
        .await?;

    for user in loaded {
        match &user.name[..] {
            "Alice" => assert_eq!(user.nickname.get().as_deref(), Some("ally")),
            "Bob" => assert_eq!(user.nickname.get().as_deref(), None),
            "Carol" => assert_eq!(user.nickname.get().as_deref(), None),
            other => panic!("unexpected user {other}"),
        }
    }

    Ok(())
}
