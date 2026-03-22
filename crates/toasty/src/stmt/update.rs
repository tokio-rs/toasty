use super::Query;
use crate::{
    schema::{Load, Model},
    Executor, Result,
};
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

/// A typed update statement.
///
/// `Update` modifies records matching a selection (typically derived from a
/// [`Query`]). Field assignments are added with [`set`](Update::set),
/// [`insert`](Update::insert), and [`remove`](Update::remove).
///
/// The type parameter `T` is the **returning type** — it determines what
/// `exec()` produces, not which model is being updated. For example,
/// `Update<User>` returns the updated `User` (single-row update), while
/// `Update<List<User>>` returns the updated records as `Vec<User>`.
///
/// Generated update-builders wrap this type and expose typed setter methods.
/// You rarely construct `Update` by hand.
///
/// By default, an update returns the changed records. Call
/// [`set_returning_none`](Update::set_returning_none) to suppress this.
pub struct Update<T> {
    pub(crate) untyped: stmt::Update,
    _p: PhantomData<T>,
}

// Methods available on all Update<M> regardless of M
impl<T> Update<T> {
    /// Create a new update statement from the given query selection.
    ///
    /// All records matched by `selection` will be updated. By default the
    /// update returns the changed records (`Returning::Changed`).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// ```
    pub fn new(selection: Query<T>) -> Self {
        let mut stmt = selection.untyped.update();
        stmt.returning = Some(stmt::Returning::Changed);

        Self {
            untyped: stmt,
            _p: PhantomData,
        }
    }

    /// Wrap a raw untyped [`stmt::Update`](toasty_core::stmt::Update).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// // Round-trip through an untyped update
    /// let update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// let raw = update.into_untyped_stmt();
    /// ```
    pub const fn from_untyped(untyped: stmt::Update) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Get a mutable reference to the underlying untyped update.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// let raw = update.as_untyped_mut();
    /// // Inspect or modify the raw update
    /// assert!(raw.returning.is_some());
    /// ```
    pub fn as_untyped_mut(&mut self) -> &mut stmt::Update {
        &mut self.untyped
    }

    /// Replace all field assignments with `assignments`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// update.set_assignments(toasty_core::stmt::Assignments::default());
    /// ```
    pub fn set_assignments(&mut self, assignments: stmt::Assignments) {
        self.untyped.assignments = assignments;
    }

    /// Assign a value to a field.
    ///
    /// `field` identifies which field to update and `expr` is the new value.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// // Set field at index 1 (name) to "Bob"
    /// update.set(1, toasty_core::stmt::Value::from("Bob"));
    /// ```
    pub fn set(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.set(field, expr);
    }

    /// Append a value to a collection field (e.g., a has-many relation).
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// update.insert(1, toasty_core::stmt::Value::from("new_tag"));
    /// ```
    pub fn insert(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.insert(field, expr);
    }

    /// Remove a value from a collection field.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// update.remove(1, toasty_core::stmt::Value::from("old_tag"));
    /// ```
    pub fn remove(&mut self, field: impl Into<stmt::Projection>, expr: impl Into<stmt::Expr>) {
        self.untyped.assignments.remove(field, expr);
    }

    /// Don't return anything.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// update.set_returning_none();
    /// ```
    pub fn set_returning_none(&mut self) {
        self.untyped.returning = None;
    }

    /// Consume this typed update and return the untyped core statement.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// let _raw = update.into_untyped_stmt();
    /// ```
    pub fn into_untyped_stmt(self) -> stmt::Statement {
        self.untyped.into()
    }
}

impl<T: Load> Update<T> {
    /// Execute this update statement against the given executor.
    ///
    /// Returns the updated records unless
    /// [`set_returning_none`](Update::set_returning_none) was called.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// # let mut db = toasty::Db::builder().register::<User>().build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// use toasty::stmt::{List, Query, Update};
    ///
    /// let mut update = Update::<List<User>>::new(Query::<List<User>>::all());
    /// update.set(1, toasty_core::stmt::Value::from("Bob"));
    /// let _users: Vec<User> = update.exec(&mut db).await.unwrap();
    /// # });
    /// ```
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T::Output> {
        executor.exec(self.into()).await
    }
}

impl<M> Clone for Update<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M: Model> Default for Update<M> {
    fn default() -> Self {
        Self {
            untyped: stmt::Update {
                target: stmt::UpdateTarget::Model(M::id()),
                assignments: stmt::Assignments::default(),
                filter: stmt::Filter::new(stmt::Expr::from(false)),
                condition: stmt::Condition::default(),
                returning: Some(stmt::Returning::Changed),
            },
            _p: PhantomData,
        }
    }
}

impl<M> AsRef<stmt::Update> for Update<M> {
    fn as_ref(&self) -> &stmt::Update {
        &self.untyped
    }
}

impl<M> AsMut<stmt::Update> for Update<M> {
    fn as_mut(&mut self) -> &mut stmt::Update {
        &mut self.untyped
    }
}

impl<M> fmt::Debug for Update<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
