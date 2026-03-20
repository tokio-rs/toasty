use super::{IntoExpr, IntoStatement, List, Path, Statement};
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

/// A typed handle to a model association (relation).
///
/// `Association` represents a link between a source model and a target model,
/// such as a has-many or belongs-to relation. It wraps an untyped
/// [`stmt::Association`](toasty_core::stmt::Association) and carries a type `T`
/// that encodes the target:
///
/// - `Association<List<M>>` — a has-many relation returning multiple `M` records.
/// - `Association<M>` — a has-one or belongs-to relation returning a single `M`.
///
/// Associations are constructed by generated code (see [`many`](Association::many),
/// [`many_via_one`](Association::many_via_one), and [`one`](Association::one)).
/// They implement [`IntoStatement`] so they can be passed directly to
/// [`Db::exec`](crate::Db::exec).
pub struct Association<T> {
    pub(crate) untyped: stmt::Association,
    _p: PhantomData<T>,
}

impl<M: Model> Association<List<M>> {
    /// Create a has-many association from `source` following `path`.
    ///
    /// # Panics
    ///
    /// Panics if the root of `path` does not match the model id of `T`.
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
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     user_id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Association, Path, List, Query};
    ///
    /// let source = Query::<User>::filter(User::fields().id().eq(1));
    /// let path = Path::<User, List<Todo>>::from_field_index(2);
    /// let _assoc = Association::many(source, path);
    /// ```
    pub fn many<T: Model>(source: super::Query<List<T>>, path: Path<T, List<M>>) -> Self {
        assert_eq!(path.untyped.root.expect_model(), T::id());

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    /// Create a has-many association through a singular (has-one / belongs-to)
    /// path. Because the source is a query that may match multiple rows, the
    /// result is still a list.
    ///
    /// # Panics
    ///
    /// Panics if the root of `path` does not match the model id of `T`.
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
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     user_id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Association, Path, List, Query};
    ///
    /// let source = Query::<Todo>::all();
    /// let path = Path::<Todo, User>::from_field_index(1);
    /// let _assoc: Association<List<User>> = Association::many_via_one(source, path);
    /// ```
    pub fn many_via_one<T: Model>(source: super::Query<List<T>>, path: Path<T, M>) -> Self {
        assert_eq!(path.untyped.root.expect_model(), T::id());

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }

    /// Insert associated records into this has-many relation.
    ///
    /// Converts the association into an update statement that adds `expr` to
    /// the relation's field on the source model.
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
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     user_id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Association, Insert, Path, List, Query};
    ///
    /// let source = Query::<User>::filter(User::fields().id().eq(1));
    /// let path = Path::<User, List<Todo>>::from_field_index(2);
    /// let assoc = Association::many(source, path);
    ///
    /// let new_todo = Insert::<Todo>::blank_single();
    /// let _stmt = assoc.insert(new_todo.into_list_expr());
    /// ```
    pub fn insert(self, expr: impl IntoExpr<List<M>>) -> Statement<M> {
        let [index] = self.untyped.path.projection.as_slice() else {
            todo!()
        };

        let mut stmt = self.untyped.source.update();
        stmt.assignments.insert(*index, expr.into_expr().untyped);

        Statement {
            untyped: stmt.into(),
            _p: PhantomData,
        }
    }

    /// Remove an associated record from this has-many relation.
    ///
    /// Converts the association into an update statement that removes `expr`
    /// from the relation's field on the source model.
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
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     user_id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Association, Expr, Path, List, Query};
    ///
    /// let source = Query::<User>::filter(User::fields().id().eq(1));
    /// let path = Path::<User, List<Todo>>::from_field_index(2);
    /// let assoc = Association::many(source, path);
    ///
    /// // Remove a todo by its expression
    /// let todo_expr = Expr::<Todo>::from_untyped(
    ///     toasty_core::stmt::Value::from(42_i64),
    /// );
    /// let _stmt = assoc.remove(todo_expr);
    /// ```
    pub fn remove(self, expr: impl IntoExpr<M>) -> Statement<M> {
        let [index] = self.untyped.path.projection.as_slice() else {
            todo!()
        };
        let mut stmt = self.untyped.source.update();
        stmt.assignments.remove(*index, expr.into_expr().untyped);

        Statement {
            untyped: stmt.into(),
            _p: PhantomData,
        }
    }
}

impl<T: Model> IntoStatement for Association<List<T>> {
    type Returning = List<T>;

    fn into_statement(self) -> Statement<List<T>> {
        let query = stmt::Query::builder(stmt::SourceModel {
            model: T::id(),
            via: Some(self.untyped),
        })
        .build();
        Statement::from_untyped_stmt(query.into())
    }
}

impl<M: Model> Association<M> {
    /// Create a has-one or belongs-to association from `source` following
    /// `path`.
    ///
    /// # Panics
    ///
    /// Panics if the root of `path` does not match the model id of `T`.
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
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     id: i64,
    /// #     user_id: i64,
    /// #     title: String,
    /// # }
    /// use toasty::stmt::{Association, Path, Query};
    ///
    /// let source = Query::<Todo>::filter(Todo::fields().id().eq(1));
    /// let path = Path::<Todo, User>::from_field_index(1);
    /// let _assoc = Association::one(source, path);
    /// ```
    pub fn one<T: Model>(source: super::Query<List<T>>, path: Path<T, M>) -> Self {
        assert_eq!(path.untyped.root.expect_model(), T::id());

        Self {
            untyped: stmt::Association {
                source: Box::new(source.untyped),
                path: path.untyped,
            },
            _p: PhantomData,
        }
    }
}

impl<T: Model> IntoStatement for Association<T> {
    type Returning = List<T>;

    fn into_statement(self) -> Statement<List<T>> {
        let query = stmt::Query::builder(stmt::SourceModel {
            model: T::id(),
            via: Some(self.untyped),
        })
        .build();
        Statement::from_untyped_stmt(query.into())
    }
}

impl<M> fmt::Debug for Association<M> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(fmt)
    }
}
