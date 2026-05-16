//! Regression tests for `has_many` / `belongs_to` relationships whose
//! foreign key spans multiple columns.
//!
//! See: https://github.com/tokio-rs/toasty/discussions/904

use crate::prelude::*;

/// When a composite-key `belongs_to` has no covering index on the parent
/// side, schema verification must return a helpful invalid-schema error
/// rather than panicking with `failed to find relation index`.
#[driver_test]
pub async fn composite_belongs_to_missing_index_is_error(test: &mut Test) -> Result<()> {
    #[derive(Debug, toasty::Model)]
    #[key(id, revision)]
    struct Parent {
        id: String,
        revision: i64,

        #[has_many]
        children: toasty::HasMany<Child>,
    }

    #[derive(Debug, toasty::Model)]
    struct Child {
        #[key]
        id: String,

        // Two field-level `#[index]` annotations create two separate
        // single-column indexes — neither covers the composite foreign key.
        #[index]
        parent_id: String,
        #[index]
        parent_revision: i64,

        #[belongs_to(key = [parent_id, parent_revision], references = [id, revision])]
        parent: toasty::BelongsTo<Parent>,
    }

    let err = test
        .try_setup_db(models!(Parent, Child))
        .await
        .expect_err("schema verification should reject this layout");

    assert!(
        err.is_invalid_schema(),
        "expected invalid_schema error, got: {err}",
    );

    let msg = err.to_string();
    assert!(
        msg.contains("parent_id") && msg.contains("parent_revision"),
        "error should mention the foreign-key fields, got: {msg}",
    );
    assert!(
        msg.contains("#[index(parent_id, parent_revision)]"),
        "error should suggest adding a composite index, got: {msg}",
    );
    // Both FK fields are individually `#[index]`-annotated, so the verifier
    // should detect that and explain why two single-column indexes don't
    // satisfy a composite foreign key.
    assert!(
        msg.contains("each foreign-key field already has its own `#[index]`"),
        "error should call out the per-field `#[index]` annotations, got: {msg}",
    );

    Ok(())
}
