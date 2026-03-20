use super::Query;
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

/// A typed update statement for model `M`.
///
/// `Update` modifies records matching a selection (typically derived from a
/// [`Query`]). Field assignments are added with [`set`](Update::set),
/// [`insert`](Update::insert), and [`remove`](Update::remove).
///
/// Generated update-builders wrap this type and expose typed setter methods.
/// You rarely construct `Update` by hand.
///
/// By default, an update returns the changed records. Call
/// [`set_returning_none`](Update::set_returning_none) to suppress this.
pub struct Update<M> {
    pub(crate) untyped: stmt::Update,
    _p: PhantomData<M>,
}

impl<M: Model> Update<M> {
    /// Create an update that targets the records matched by `selection`.
    ///
    /// The update is initially empty (no assignments). Add field assignments
    /// with [`set`](Update::set) before executing.
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let selection = Query::<User>::filter(User::fields().id().eq(1));
    /// let _update = Update::<User>::new(selection);
    /// ```
    pub fn new(mut selection: Query<M>) -> Self {
        if let stmt::ExprSet::Values(values) = &mut selection.untyped.body {
            let rows = std::mem::take(&mut values.rows);
            let filter = stmt::Expr::in_list(stmt::Expr::ref_ancestor_model(0), rows);
            selection.untyped.body =
                stmt::ExprSet::Select(Box::new(stmt::Select::new(M::id(), filter)));
        }

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
    /// use toasty::stmt::{Query, Update};
    ///
    /// // Round-trip through an untyped update
    /// let update = Update::<User>::new(Query::<User>::all());
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let mut update = Update::<User>::new(Query::<User>::all());
    /// let raw = update.as_untyped_mut();
    /// // Inspect or modify the raw update
    /// assert!(raw.returning.is_some());
    /// ```
    pub fn as_untyped_mut(&mut self) -> &mut stmt::Update {
        &mut self.untyped
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let mut update = Update::<User>::new(Query::<User>::all());
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let mut update = Update::<User>::new(Query::<User>::all());
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let mut update = Update::<User>::new(Query::<User>::all());
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let mut update = Update::<User>::new(Query::<User>::all());
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
    /// use toasty::stmt::{Query, Update};
    ///
    /// let update = Update::<User>::new(Query::<User>::all());
    /// let _raw = update.into_untyped_stmt();
    /// ```
    pub fn into_untyped_stmt(self) -> stmt::Statement {
        self.untyped.into()
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
