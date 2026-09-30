use crate::prelude::*;

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn has_one_membership_with_null_foreign_key(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User::[
        { name: "missing" },
        { name: "present", profile: { bio: "associated" } },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Profile {
        bio: "unassociated"
    })
    .exec(&mut db)
    .await?;

    let users = User::filter(User::fields().profile().in_query(Profile::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(users, [{ name: "missing" }]);

    let users = User::filter(User::fields().profile().in_query(Profile::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(users, [{ name: "present" }]);

    Ok(())
}

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn belongs_to_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    toasty::create!(User {
        name: "present",
        profile: { bio: "associated" },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Profile {
        bio: "unassociated"
    })
    .exec(&mut db)
    .await?;

    let profiles = Profile::filter(Profile::fields().user().in_query(User::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    let profiles = Profile::filter(Profile::fields().user().in_query(User::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(profiles, [{ bio: "associated" }]);

    let profiles = Profile::filter(
        Profile::fields()
            .user()
            .profile()
            .in_query(Profile::all())
            .not(),
    )
    .exec(&mut db)
    .await?;
    assert_struct!(profiles, [{ bio: "unassociated" }]);

    Ok(())
}

#[driver_test(requires(sql))]
pub async fn membership_with_composite_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Source {
        id: String,
        revision: i64,

        #[has_one]
        document: Option<Document>,
    }

    #[derive(Debug, toasty::Model)]
    #[unique(source_id, source_revision)]
    struct Document {
        #[key]
        id: String,

        source_id: Option<String>,
        source_revision: Option<i64>,

        #[belongs_to(key = [source_id, source_revision], references = [id, revision])]
        source: toasty::Deferred<Option<Source>>,
    }

    let mut db = t.setup_db(models!(Source, Document)).await;
    toasty::create!(Source::[
        { id: "source", revision: 1 },
        { id: "source", revision: 2, document: { id: "document" } },
    ])
    .exec(&mut db)
    .await?;
    toasty::create!(Document { id: "unassociated" })
        .exec(&mut db)
        .await?;

    let sources = Source::filter(Source::fields().document().in_query(Document::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ revision: 1 }]);

    let sources = Source::filter(Source::fields().document().in_query(Document::all()))
        .exec(&mut db)
        .await?;
    assert_struct!(sources, [{ revision: 2 }]);

    let documents = Document::filter(Document::fields().source().in_query(Source::all()).not())
        .exec(&mut db)
        .await?;
    assert_struct!(documents, [{ id: "unassociated" }]);

    Ok(())
}

#[driver_test(
    requires(scan),
    scenario(crate::scenarios::has_one_optional_belongs_to::id_uuid)
)]
pub async fn negated_direct_belongs_to_membership(t: &mut Test) -> Result<()> {
    let mut db = setup(t).await;
    let user = toasty::create!(User {
        name: "present",
        profile: { bio: "associated" },
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Profile {
        bio: "unassociated"
    })
    .exec(&mut db)
    .await?;

    let found = Profile::filter(
        Profile::fields()
            .user()
            .in_query(User::filter_by_id(user.id))
            .not(),
    )
    .exec(&mut db)
    .await?;
    assert_struct!(found, [{ bio: "unassociated" }]);
    Ok(())
}

#[driver_test(id(ID), requires(scan))]
pub async fn limited_membership_preserves_candidates(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,
        #[has_one]
        document: toasty::Deferred<Option<Document>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = position)]
    struct Document {
        group: String,
        position: i64,
        #[unique]
        user_id: Option<ID>,
        #[belongs_to(key = user_id, references = id)]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = t.setup_db(models!(User, Document)).await;
    let user = User::create().exec(&mut db).await?;
    toasty::create!(Document::[
        { group: "selected", position: 0 },
        { group: "selected", position: 1, user_id: user.id },
    ])
    .exec(&mut db)
    .await?;

    for offset in [0, 1, 2] {
        let candidates = Document::filter_by_group("selected")
            .order_by(Document::fields().position().asc())
            .limit(1)
            .offset(offset);
        let filter = User::fields().document().in_query(candidates);
        t.log().clear();
        let found = User::filter(filter.clone()).exec(&mut db).await?;
        let matches = usize::from(offset == 1);
        assert_eq!(found.len(), matches);
        assert_limited_membership_reads(t, matches);

        t.log().clear();
        let excluded = User::filter(filter.not()).exec(&mut db).await?;
        assert_eq!(excluded.len(), 1 - matches);
        assert_limited_membership_reads(t, 1);
    }

    let candidates = Document::filter_by_group("selected")
        .filter(toasty::stmt::Expr::from_untyped(false))
        .limit(1);
    let filter = User::fields().document().in_query(candidates);
    t.log().clear();
    assert!(User::filter(filter.clone()).exec(&mut db).await?.is_empty());
    assert_eq!(t.log().len(), 0);
    t.log().clear();
    assert_eq!(User::filter(filter.not()).exec(&mut db).await?.len(), 1);
    assert_eq!(t.log().len(), 1);

    let candidates = Document::filter_by_group("selected")
        .filter(Document::fields().user().in_query(User::all()))
        .order_by(Document::fields().position().asc())
        .limit(1);
    t.log().clear();
    let found = User::filter(User::fields().document().in_query(candidates))
        .exec(&mut db)
        .await?;
    assert_eq!(found.len(), 1);
    assert_eq!(t.log().len(), if t.capability().sql() { 1 } else { 3 });

    Ok(())
}

#[driver_test(requires(scan))]
pub async fn limited_membership_with_composite_key(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = id)]
    struct User {
        group: String,
        id: String,
        #[has_many]
        documents: toasty::Deferred<Vec<Document>>,
    }

    #[derive(Debug, toasty::Model)]
    #[key(partition = group, local = position)]
    #[index(user_group, user_id)]
    struct Document {
        group: String,
        position: i64,
        user_group: Option<String>,
        user_id: Option<String>,
        #[belongs_to(key = [user_group, user_id], references = [group, id])]
        user: toasty::Deferred<Option<User>>,
    }

    let mut db = t.setup_db(models!(User, Document)).await;
    toasty::create!(User {
        group: "selected",
        id: "user"
    })
    .exec(&mut db)
    .await?;
    toasty::create!(Document::[
        { group: "selected", position: 0 },
        { group: "selected", position: 1, user_group: "selected" },
        { group: "selected", position: 2, user_group: "selected", user_id: "user" },
    ])
    .exec(&mut db)
    .await?;

    for offset in 0..4 {
        let candidates = Document::filter_by_group("selected")
            .order_by(Document::fields().position().asc())
            .limit(1)
            .offset(offset);
        let path: toasty::stmt::Path<User, toasty::stmt::List<Document>> =
            User::fields().documents().into();
        let path: toasty_core::stmt::Path = path.into();
        let filter = toasty::stmt::Expr::from_untyped(toasty_core::stmt::Expr::in_subquery(
            path.into_stmt(),
            toasty::stmt::IntoStatement::into_statement(candidates)
                .into_untyped()
                .into_query_unwrap(),
        ));
        let matches = usize::from(offset == 2);

        t.log().clear();
        let found = User::filter(filter.clone()).exec(&mut db).await?;
        assert_eq!(found.len(), matches);
        assert_limited_membership_reads(t, matches);

        t.log().clear();
        let excluded = User::filter(filter.not()).exec(&mut db).await?;
        assert_eq!(excluded.len(), 1 - matches);
        assert_limited_membership_reads(t, 1);
    }
    Ok(())
}

fn assert_limited_membership_reads(t: &Test, outer_reads: usize) {
    use toasty_core::driver::Operation;

    if t.capability().sql() {
        assert_eq!(t.log().len(), 1);
        assert!(matches!(t.log().pop_op(), Operation::QuerySql(_)));
    } else {
        assert_eq!(t.log().len(), 1 + outer_reads);
        assert!(matches!(t.log().pop_op(), Operation::QueryPk(_)));
        if outer_reads != 0 {
            assert!(matches!(
                t.log().pop_op(),
                Operation::GetByKey(_) | Operation::Scan(_)
            ));
        }
    }
}
