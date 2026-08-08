use super::{Insert, Statement};
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

/// A typed single-row upsert statement for model `M`.
///
/// `#[derive(Model)]` generates an `upsert_by_*` constructor for each primary
/// key and unique constraint. The generated builder owns an `Upsert<M>` and
/// exposes typed field setters, `on_create`, `on_update`, `or_ignore`, and
/// `exec`. Users normally work through that builder instead of constructing
/// this type directly.
///
/// An ordinary replacement setter supplies a value for both branches. A shared
/// mutation applies to the field's `#[default]` value when the row is absent
/// and to the stored value when the selected constraint matches. The
/// branch-specific closures override that behavior for fields that need
/// different create and update values. A regular execution returns the record
/// stored by the database; `or_ignore` returns `Some(M)` after an insert and
/// `None` after a conflict.
///
/// The database executes the conflict check and mutation atomically. Backend
/// support differs: PostgreSQL, SQLite, and Turso accept primary-key and unique
/// targets plus branch-specific assignments; DynamoDB accepts primary-key
/// targets and create-only assignments for required fields, but not
/// update-only assignments; MySQL does not support this targeted upsert API.
///
/// # Examples
///
/// ```
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     #[unique]
/// #     email: String,
/// #     name: String,
/// # }
/// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
/// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
/// # db.push_schema().await.unwrap();
/// let created = User::upsert_by_email("alice@example.com")
///     .name("Alice")
///     .exec(&mut db)
///     .await
///     .unwrap();
///
/// let updated = User::upsert_by_email("alice@example.com")
///     .name("Alicia")
///     .exec(&mut db)
///     .await
///     .unwrap();
///
/// assert_eq!(updated.id, created.id);
/// assert_eq!(updated.name, "Alicia");
/// # });
/// ```
pub struct Upsert<M> {
    pub(crate) untyped: stmt::Insert,
    _p: PhantomData<M>,
}

impl<M: Model> Upsert<M> {
    /// Creates a blank upsert targeting the specified model fields.
    #[doc(hidden)]
    pub fn blank(target: impl IntoIterator<Item = usize>) -> Self {
        let mut insert = Insert::<M>::blank_single().untyped;
        insert.upsert = Some(Box::new(stmt::Upsert {
            target: stmt::UpsertTarget::Fields(
                target
                    .into_iter()
                    .map(stmt::Projection::from_index)
                    .collect(),
            ),
            shared: stmt::Assignments::new(),
            defaults: stmt::Assignments::new(),
            update_defaults: stmt::Assignments::new(),
            create: stmt::Assignments::new(),
            update: stmt::Assignments::new(),
            action: stmt::UpsertAction::Update,
        }));
        Self {
            untyped: insert,
            _p: PhantomData,
        }
    }

    /// Returns the untyped insert AST owned by this wrapper.
    #[doc(hidden)]
    pub fn untyped_mut(&mut self) -> &mut stmt::Insert {
        &mut self.untyped
    }

    /// Consumes the wrapper and returns its untyped statement.
    #[doc(hidden)]
    pub fn into_untyped(self) -> stmt::Statement {
        self.untyped.into()
    }
}

impl<M> From<Upsert<M>> for Statement<M> {
    fn from(value: Upsert<M>) -> Self {
        Statement::from_untyped_stmt(value.untyped.into())
    }
}

impl<M> Clone for Upsert<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> fmt::Debug for Upsert<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
