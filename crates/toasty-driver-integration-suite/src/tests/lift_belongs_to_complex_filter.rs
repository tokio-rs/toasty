//! Regression test for `lift_belongs_to_in_subquery` silently dropping
//! filter constraints whose operands are neither side an `Expr::Reference`.
//!
//! Before the fix, the visitor's catch-all match arm did nothing — leaving
//! `fail = false` and `operands = []`. The lift then produced an empty
//! `ExprAnd` (= `true`), so the inner WHERE clause was discarded and the
//! IN subquery returned all rows of the target model.
//!
//! Embedded struct field access inside an `in_query` produces such a
//! constraint, since `address.city` lowers to `Project(ref(address), [city])`.

use crate::prelude::*;

#[driver_test(id(ID), requires(sql))]
pub async fn lift_belongs_to_preserves_embedded_field_filter(t: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Embed)]
    struct Address {
        city: String,
    }

    #[derive(Debug, toasty::Model)]
    struct User {
        #[key]
        #[auto]
        id: ID,

        address: Address,
    }

    #[derive(Debug, toasty::Model)]
    struct Post {
        #[key]
        #[auto]
        id: ID,

        title: String,

        #[index]
        user_id: ID,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    let mut db = t.setup_db(models!(User, Post, Address)).await;

    toasty::create!(Post::[
        {
            title: "Seattle post",
            user: { address: Address { city: "Seattle".into() } },
        },
        {
            title: "NYC post",
            user: { address: Address { city: "NYC".into() } },
        },
    ])
    .exec(&mut db)
    .await?;

    // The inner filter `address.city = "Seattle"` is a `Project` over a
    // reference, not a bare reference. Before the fix, the visitor silently
    // dropped this constraint and the IN subquery matched every user.
    let posts: Vec<Post> = Post::filter(
        Post::fields()
            .user()
            .in_query(User::filter(User::fields().address().city().eq("Seattle"))),
    )
    .exec(&mut db)
    .await?;

    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].title, "Seattle post");

    Ok(())
}
