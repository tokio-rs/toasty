//! `IN`-list lowering where the list's left side projects into an embedded
//! field. The simplifier folds `x == a OR x == b` into `x IN (a, b)`, so an OR
//! over `name._0` lowers to `name._0 IN (...)` — an `IN` list whose LHS is a
//! projection, not a plain column reference.

use crate::prelude::*;

#[derive(Debug, toasty::Embed)]
struct Name(String);

/// An embedded newtype field referenced twice under an OR — folds to
/// `name._0 IN (...)`, exercising the `Expr::Project` LHS in `lower_expr_in_list`.
#[driver_test(requires(sql))]
pub async fn or_over_embedded_field_folds_to_in_list(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Topic {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: Name,
    }

    let mut db = t.setup_db(models!(Topic)).await;

    toasty::create!(Topic::[
        { name: Name("one".into()) },
        { name: Name("two".into()) },
        { name: Name("three".into()) },
    ])
    .exec(&mut db)
    .await?;

    let topics: Vec<Topic> = Topic::filter(
        Topic::fields()
            .name()
            ._0()
            .eq("one")
            .or(Topic::fields().name()._0().eq("two")),
    )
    .exec(&mut db)
    .await?;

    assert_eq_unordered!(topics.iter().map(|t| &t.name.0[..]), ["one", "two"]);

    Ok(())
}

/// The same fold reached through a `.any()` over a `BelongsTo → HasMany` chain:
/// the `name._0 IN (...)` lands inside the lifted subquery.
#[driver_test(requires(sql))]
pub async fn any_with_or_over_embedded_field(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    struct Project {
        #[key]
        #[auto]
        id: uuid::Uuid,

        #[has_many]
        topics: toasty::Deferred<Vec<Topic>>,

        #[has_many]
        releases: toasty::Deferred<Vec<Release>>,
    }

    #[derive(Debug, toasty::Model)]
    struct Topic {
        #[key]
        #[auto]
        id: uuid::Uuid,

        name: Name,

        #[index]
        project_id: uuid::Uuid,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::Deferred<Project>,
    }

    #[derive(Debug, toasty::Model)]
    struct Release {
        #[key]
        #[auto]
        id: uuid::Uuid,

        label: String,

        #[index]
        project_id: uuid::Uuid,

        #[belongs_to(key = project_id, references = id)]
        project: toasty::Deferred<Project>,
    }

    let mut db = t.setup_db(models!(Project, Topic, Release)).await;

    toasty::create!(Project {
        topics: [{ name: Name("one".into()) }],
        releases: [{ label: "r-match" }],
    })
    .exec(&mut db)
    .await?;

    toasty::create!(Project {
        topics: [{ name: Name("nope".into()) }],
        releases: [{ label: "r-other" }],
    })
    .exec(&mut db)
    .await?;

    // Releases whose project has a topic named "one" or "two".
    let releases: Vec<Release> = Release::filter(
        Release::fields().project().topics().any(
            Topic::fields()
                .name()
                ._0()
                .eq("one")
                .or(Topic::fields().name()._0().eq("two")),
        ),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].label, "r-match");

    Ok(())
}
