//! Regression test for https://github.com/tokio-rs/toasty/issues/1046
//!
//! A crate that enforces `#![deny(missing_docs)]` must still be able to derive
//! `toasty::Model`. The derive emits public inherent methods (`create`, `all`,
//! `filter`, `fields`, `update`, `delete`, per-field filter methods, relation
//! accessors, …); each carries a generated doc comment so it does not trip the
//! lint. If this file fails to compile, a generated method lost its docs.
#![deny(missing_docs)]

/// A user, exercising key, unique, indexed, and `has_many` fields so every
/// generated inherent method kind is present on the model.
#[derive(Debug, toasty::Model)]
pub struct User {
    /// Primary key.
    #[key]
    #[auto]
    pub id: u64,

    /// Unique email — generates `get_by_email` / `filter_by_email`.
    #[unique]
    pub email: String,

    /// Indexed name — generates `filter_by_name`.
    #[index]
    pub name: String,

    /// Posts authored by this user.
    #[has_many]
    pub posts: toasty::Deferred<Vec<Post>>,
}

/// A post that belongs to a user, exercising the `belongs_to` relation
/// accessor codegen.
#[derive(Debug, toasty::Model)]
pub struct Post {
    /// Primary key.
    #[key]
    #[auto]
    pub id: u64,

    /// Author id.
    #[index]
    pub user_id: u64,

    /// Author.
    #[belongs_to(key = user_id, references = id)]
    pub user: toasty::Deferred<User>,
}

/// If this compiles, the derive-generated public methods do not trip
/// `#![deny(missing_docs)]`.
#[test]
fn models_compile_under_deny_missing_docs() {}
