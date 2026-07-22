use crate::prelude::*;

/// Tuple and chained ordering apply independently to each parent's relation.
#[driver_test(
    id(ID),
    requires(sql),
    scenario(crate::scenarios::has_many_belongs_to_with_flags)
)]
pub async fn preload_has_many_ordered_per_parent(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    toasty::create!(User::[
        {
            name: "alice",
            todos: [
                { title: "b", complete: false, priority: 2 },
                { title: "a", complete: false, priority: 2 },
                { title: "c", complete: false, priority: 1 },
            ]
        },
        {
            name: "bob",
            todos: [
                { title: "z", complete: false, priority: 1 },
                { title: "y", complete: false, priority: 1 },
                { title: "x", complete: false, priority: 3 },
            ]
        },
    ])
    .exec(&mut db)
    .await?;

    let tuple_ordered: Vec<User> = User::all()
        .order_by(User::fields().name().asc())
        .include(User::fields().todos().order_by((
            Todo::fields().priority().desc(),
            Todo::fields().title().asc(),
        )))
        .exec(&mut db)
        .await?;

    assert_struct!(tuple_ordered, [
        { name: "alice", todos.get(): [{ title: "a" }, { title: "b" }, { title: "c" }] },
        { name: "bob", todos.get(): [{ title: "x" }, { title: "y" }, { title: "z" }] },
    ]);

    let chained_ordered: Vec<User> = User::all()
        .order_by(User::fields().name().asc())
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().priority().desc())
                .order_by(Todo::fields().title().asc()),
        )
        .exec(&mut db)
        .await?;

    assert_struct!(chained_ordered, [
        { name: "alice", todos.get(): [{ title: "a" }, { title: "b" }, { title: "c" }] },
        { name: "bob", todos.get(): [{ title: "x" }, { title: "y" }, { title: "z" }] },
    ]);

    Ok(())
}

/// Include filters run before ordering.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_filter_and_order(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "c" }, { title: "b" }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .todos()
                .filter(Todo::fields().title().ne("c"))
                .order_by(Todo::fields().title().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.todos.get(), [
        { title: "b" },
        { title: "a" },
    ]);

    Ok(())
}

/// Ordering applies at each terminal relation in a nested include path.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::user_post_comment))]
pub async fn preload_nested_relations_ordered_at_each_level(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        posts: [
            {
                title: "b",
                comments: [{ body: "a" }, { body: "c" }, { body: "b" }]
            },
            {
                title: "a",
                comments: [{ body: "x" }, { body: "z" }, { body: "y" }]
            },
        ]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .posts()
                .order_by(Post::fields().title().asc()),
        )
        .include(
            User::fields()
                .posts()
                .comments()
                .order_by(Comment::fields().body().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.posts.get(), [
        { title: "a", comments.get(): [{ body: "z" }, { body: "y" }, { body: "x" }] },
        { title: "b", comments.get(): [{ body: "c" }, { body: "b" }, { body: "a" }] },
    ]);

    Ok(())
}

/// Include modifiers take precedence over fields with the same names.
#[driver_test(requires(sql))]
pub async fn preload_modifiers_with_reserved_field_names(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Parent {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        children: toasty::Deferred<Vec<Child>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        #[auto]
        id: uuid::Uuid,

        filter: String,
        order_by: String,
        rank: i64,

        #[index]
        parent_id: uuid::Uuid,

        #[belongs_to(key = parent_id, references = id)]
        parent: toasty::Deferred<Parent>,
    }

    let mut db = test.setup_db(models!(Parent, Child)).await;

    let parent = toasty::create!(Parent {
        children: [
            { filter: "a", order_by: "a", rank: 1 },
            { filter: "b", order_by: "b", rank: 3 },
            { filter: "c", order_by: "c", rank: 2 },
        ]
    })
    .exec(&mut db)
    .await?;

    let parent = Parent::filter_by_id(parent.id)
        .include(
            Parent::fields()
                .children()
                .filter(Child::fields().rank().gt(1))
                .order_by(Child::fields().rank().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(parent.children.get(), [
        { rank: 3 },
        { rank: 2 },
    ]);

    Ok(())
}

/// The last include supplies the complete ordering for a repeated relation path.
#[driver_test(id(ID), requires(sql), scenario(crate::scenarios::has_many_belongs_to))]
pub async fn preload_has_many_repeated_ordering_last_wins(test: &mut Test) -> Result<()> {
    use toasty_core::{driver::Operation, stmt::Statement};

    let mut db = setup(test).await;

    let user = toasty::create!(User {
        name: "alice",
        todos: [{ title: "a" }, { title: "c" }, { title: "b" }]
    })
    .exec(&mut db)
    .await?;

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().asc()),
        )
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().desc()),
        )
        .get(&mut db)
        .await?;

    assert_struct!(loaded.todos.get(), [
        { title: "c" },
        { title: "b" },
        { title: "a" },
    ]);

    test.log().clear();

    let loaded = User::filter_by_id(user.id)
        .include(
            User::fields()
                .todos()
                .order_by(Todo::fields().title().desc()),
        )
        .include(User::fields().todos())
        .get(&mut db)
        .await?;

    assert_eq!(loaded.todos.get().len(), 3);

    let _ = test.log().pop_last_op(); // transaction commit
    assert_struct!(test.log().pop_last_op(), Operation::QuerySql({
        stmt: Statement::Query({ order_by: None, .. }),
        ..
    }));

    Ok(())
}

/// Include ordering is rejected when the terminal relation is singular.
#[driver_test(id(ID), scenario(crate::scenarios::has_one_optional_belongs_to))]
pub async fn preload_has_one_ordering_rejected(test: &mut Test) -> Result<()> {
    let mut db = setup(test).await;

    let err = assert_err!(
        User::all()
            .include(
                User::fields()
                    .profile()
                    .order_by(Profile::fields().bio().asc()),
            )
            .exec(&mut db)
            .await
    );

    assert!(err.is_invalid_statement(), "unexpected error: {err}");

    Ok(())
}
