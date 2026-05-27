use crate::prelude::*;

/// Basic single-field update through an instance target.
#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn update_macro_simple(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Carl").exec(&mut db).await?;

    toasty::update!(user { name: "Carlos" })
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Carlos");

    let reloaded = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(reloaded.name, "Carlos");

    Ok(())
}

/// Multiple fields in one update call.
#[driver_test(id(ID))]
pub async fn update_macro_multiple_fields(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        name: String,
        email: String,
    }

    let mut db = test.setup_db(models!(User)).await;

    let mut user = User::create()
        .name("Carl")
        .email("carl@example.com")
        .exec(&mut db)
        .await?;

    toasty::update!(user {
        name: "Carlos",
        email: "carlos@example.com",
    })
    .exec(&mut db)
    .await?;

    assert_eq!(user.name, "Carlos");
    assert_eq!(user.email, "carlos@example.com");

    Ok(())
}

/// Field shorthand — `name` as a bare ident expands to `name: name`.
#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn update_macro_shorthand(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Carl").exec(&mut db).await?;

    let name = "Carlos";
    toasty::update!(user { name }).exec(&mut db).await?;

    assert_eq!(user.name, "Carlos");

    Ok(())
}

/// Method shorthand: `field.set(value)` lowers to
/// `field: stmt::set(value)`.
#[driver_test(id(ID), scenario(crate::scenarios::two_models))]
pub async fn update_macro_method_shorthand_set(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Carl").exec(&mut db).await?;

    toasty::update!(user { name.set("Carlos") })
        .exec(&mut db)
        .await?;

    assert_eq!(user.name, "Carlos");

    Ok(())
}

/// Method shorthand on a `Vec<scalar>` field: `tags.push("rust")`
/// lowers to `tags: stmt::push("rust")` for an atomic append.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn update_macro_method_shorthand_push(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    toasty::update!(item { tags.push("b") })
        .exec(&mut db)
        .await?;

    assert_eq!(item.tags, vec!["a".to_string(), "b".to_string()]);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, vec!["a".to_string(), "b".to_string()]);

    Ok(())
}

/// Method shorthand on a `Vec<scalar>` field: `tags.extend([..])`
/// lowers to `tags: stmt::extend([..])` for an atomic batch append.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn update_macro_method_shorthand_extend(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string()],
    })
    .exec(&mut db)
    .await?;

    toasty::update!(item { tags.extend(["b", "c"]) })
        .exec(&mut db)
        .await?;

    let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// Method shorthand on a `Vec<scalar>` field: `tags.pop()` lowers to
/// `tags: stmt::pop()` for an atomic trailing-element drop.
#[driver_test(id(ID), requires(vec_pop))]
pub async fn update_macro_method_shorthand_pop(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string(), "b".to_string(), "c".to_string()],
    })
    .exec(&mut db)
    .await?;

    toasty::update!(item { tags.pop() }).exec(&mut db).await?;

    let expected = vec!["a".to_string(), "b".to_string()];
    assert_eq!(item.tags, expected);

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert_eq!(reloaded.tags, expected);

    Ok(())
}

/// Method shorthand on a `Vec<scalar>` field: `tags.clear()` lowers to
/// `tags: stmt::clear()`, replacing the field with an empty list.
#[driver_test(id(ID), requires(vec_scalar))]
pub async fn update_macro_method_shorthand_clear(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Item {
        #[key]
        #[auto]
        id: ID,
        tags: Vec<String>,
    }

    let mut db = test.setup_db(models!(Item)).await;

    let mut item = toasty::create!(Item {
        tags: vec!["a".to_string(), "b".to_string()],
    })
    .exec(&mut db)
    .await?;

    toasty::update!(item { tags.clear() }).exec(&mut db).await?;

    assert!(item.tags.is_empty());

    let reloaded = Item::get_by_id(&mut db, &item.id).await?;
    assert!(reloaded.tags.is_empty());

    Ok(())
}

/// Update through a query builder target — no instance required.
#[driver_test(id(ID))]
pub async fn update_macro_query_target(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[index]
        name: String,
    }

    let mut db = test.setup_db(models!(User)).await;
    let user = User::create().name("Alice").exec(&mut db).await?;

    toasty::update!(User::filter_by_id(user.id) { name: "Bob" })
        .exec(&mut db)
        .await?;

    let reloaded = User::get_by_id(&mut db, &user.id).await?;
    assert_eq!(reloaded.name, "Bob");

    Ok(())
}

/// Embedded partial update via brace block: `meta: { version: 2 }`
/// lowers to `meta: stmt::apply([stmt::patch(<Metadata>::fields().version(), 2)])`.
#[driver_test(id(ID))]
pub async fn update_macro_embedded_patch(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    #[allow(dead_code)]
    struct Metadata {
        version: i64,
        status: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        #[auto]
        id: ID,
        title: String,
        meta: Metadata,
    }

    let mut db = test.setup_db(models!(Document)).await;

    let mut doc = toasty::create!(Document {
        title: "Doc",
        meta: Metadata {
            version: 1,
            status: "draft".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    toasty::update!(doc {
        meta: { version: 2, status: "published" },
    })
    .exec(&mut db)
    .await?;

    let reloaded = Document::get_by_id(&mut db, &doc.id).await?;
    assert_eq!(reloaded.meta.version, 2);
    assert_eq!(reloaded.meta.status, "published");

    Ok(())
}

/// Has-many insert via bracket-of-braces: `todos: [{ title: "x" }]`
/// lowers to
/// `todos: stmt::apply([stmt::insert(Todo::create().title("x"))])`.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn update_macro_has_many_insert(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;

    toasty::update!(user {
        todos: [{ title: "buy milk" }, { title: "walk dog" }],
    })
    .exec(&mut db)
    .await?;

    let mut titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["buy milk", "walk dog"]);

    Ok(())
}

/// Has-many list with mixed builders and plain expressions — the macro
/// passes plain entries through and wraps inline builders in
/// `stmt::insert`.
#[driver_test(id(ID), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn update_macro_has_many_mixed_list(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let mut user = User::create().name("Alice").exec(&mut db).await?;
    let old = user.todos().create().title("old").exec(&mut db).await?;

    toasty::update!(user {
        todos: [
            { title: "new" },
            toasty::stmt::remove(&old),
        ],
    })
    .exec(&mut db)
    .await?;

    let titles: Vec<_> = user
        .todos()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(titles, ["new"]);

    Ok(())
}

/// Mixing a scalar set, an embedded patch, and a method-shorthand call
/// in one invocation.
#[driver_test(id(ID))]
pub async fn update_macro_mixed_shapes(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    #[allow(dead_code)]
    struct Metadata {
        version: i64,
        status: String,
    }

    #[derive(Debug, toasty::Model)]
    #[allow(dead_code)]
    struct Document {
        #[key]
        #[auto]
        id: ID,
        title: String,
        meta: Metadata,
    }

    let mut db = test.setup_db(models!(Document)).await;

    let mut doc = toasty::create!(Document {
        title: "Doc",
        meta: Metadata {
            version: 1,
            status: "draft".to_string(),
        },
    })
    .exec(&mut db)
    .await?;

    toasty::update!(doc {
        title.set("New title"),
        meta: { version: 2 },
    })
    .exec(&mut db)
    .await?;

    let reloaded = Document::get_by_id(&mut db, &doc.id).await?;
    assert_eq!(reloaded.title, "New title");
    assert_eq!(reloaded.meta.version, 2);
    // status untouched
    assert_eq!(reloaded.meta.status, "draft");

    Ok(())
}
