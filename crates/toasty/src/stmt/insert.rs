use super::{Expr, IntoStatement, List};
use crate::schema::Model;
use std::{fmt, marker::PhantomData};
use toasty_core::stmt;

/// A typed insert statement for model `M`.
///
/// `Insert` represents one or more records to be created. Generated
/// create-builders (e.g., `User::create()`) produce `Insert` values under the
/// hood.
///
/// Field values are set with [`set`](Insert::set). Collection fields can be
/// extended with [`insert`](Insert::insert) and [`insert_all`](Insert::insert_all).
pub struct Insert<M> {
    pub(crate) untyped: stmt::Insert,
    _p: PhantomData<M>,
}

impl<M: Model> Insert<M> {
    /// Create an insert statement with a single blank record.
    ///
    /// Fields marked `#[auto]` are set to [`Expr::Default`](toasty_core::stmt::Expr::Default)
    /// (the database generates the value). All other fields are initialized to
    /// `NULL`. The caller must fill in required fields with [`set`](Insert::set)
    /// before executing; the blank record is not guaranteed to be valid on its
    /// own.
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
    /// use toasty::stmt::Insert;
    ///
    /// let mut insert = Insert::<User>::blank_single();
    /// // Fill in the required fields
    /// insert.set(0, toasty_core::stmt::Value::from(1_i64));
    /// insert.set(1, toasty_core::stmt::Value::from("Alice"));
    /// ```
    pub fn blank_single() -> Self {
        Self {
            untyped: stmt::Insert {
                target: stmt::InsertTarget::Model(M::id()),
                source: stmt::Query::new_single(vec![stmt::ExprRecord::from_vec(
                    M::schema()
                        .expect_root()
                        .fields
                        .iter()
                        .map(|field| match field.auto() {
                            Some(_) => stmt::Expr::Default,
                            None => stmt::Expr::Value(stmt::Value::Null),
                        })
                        .collect(),
                )
                .into()]),
                returning: Some(stmt::Returning::Model { include: vec![] }),
            },
            _p: PhantomData,
        }
    }

    /// Wrap a raw untyped [`stmt::Insert`](toasty_core::stmt::Insert).
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
    /// use toasty::stmt::Insert;
    /// use toasty::schema::Register;
    ///
    /// // Construct from a raw untyped insert
    /// let raw = toasty_core::stmt::Insert {
    ///     target: toasty_core::stmt::InsertTarget::Model(
    ///         <User as Register>::id(),
    ///     ),
    ///     source: toasty_core::stmt::Query::unit(),
    ///     returning: None,
    /// };
    /// let _typed = Insert::<User>::from_untyped(raw);
    /// ```
    pub const fn from_untyped(untyped: stmt::Insert) -> Self {
        Self {
            untyped,
            _p: PhantomData,
        }
    }

    /// Set the scope of the insert.
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
    /// use toasty::stmt::Insert;
    ///
    /// let mut insert = Insert::<User>::blank_single();
    /// // Scope the insert to all users (used by association inserts)
    /// insert.set_scope(User::all());
    /// ```
    pub fn set_scope<S>(&mut self, scope: S)
    where
        S: IntoStatement<Returning = List<M>>,
    {
        self.untyped.target =
            stmt::InsertTarget::Scope(Box::new(scope.into_statement().into_untyped_query()));
    }

    /// Set the value of the field at `field` index in the current record.
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
    /// use toasty::stmt::Insert;
    ///
    /// let mut insert = Insert::<User>::blank_single();
    /// insert.set(0, toasty_core::stmt::Value::from(1_i64));
    /// insert.set(1, toasty_core::stmt::Value::from("Alice"));
    /// ```
    pub fn set(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        *self.expr_mut(field) = expr.into();
    }

    /// Append a single value to the list field at `field` index.
    ///
    /// If the field is currently `NULL`, it is replaced with a new single-element
    /// list. If it is already a list, `expr` is appended.
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
    /// use toasty::stmt::Insert;
    ///
    /// let mut insert = Insert::<User>::blank_single();
    /// // Append a value to a list field (field index 1 for illustration)
    /// insert.insert(1, toasty_core::stmt::Value::from("tag1"));
    /// insert.insert(1, toasty_core::stmt::Value::from("tag2"));
    /// ```
    pub fn insert(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        // self.expr_mut(field).push(expr);
        let target = self.expr_mut(field);

        match target {
            stmt::Expr::Value(stmt::Value::Null) => {
                *target = stmt::Expr::list_from_vec(vec![expr.into()]);
            }
            stmt::Expr::List(expr_list) => {
                expr_list.items.push(expr.into());
            }
            _ => todo!("existing={target:#?}; expr={:#?}", expr.into()),
        }
    }

    /// Merge a list expression into the list field at `field` index,
    /// extending any existing list.
    ///
    /// If the field is currently `NULL`, it is replaced with `expr`. If both
    /// are lists, the items from `expr` are appended to the existing list.
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
    /// use toasty::stmt::Insert;
    ///
    /// let mut insert = Insert::<User>::blank_single();
    /// let list = toasty_core::stmt::Expr::list([
    ///     toasty_core::stmt::Expr::Value(toasty_core::stmt::Value::from("a")),
    ///     toasty_core::stmt::Expr::Value(toasty_core::stmt::Value::from("b")),
    /// ]);
    /// insert.insert_all(1, list);
    /// ```
    pub fn insert_all(&mut self, field: usize, expr: impl Into<stmt::Expr>) {
        let target = self.expr_mut(field);
        let incoming = expr.into();

        match target {
            stmt::Expr::Value(stmt::Value::Null) => {
                *target = incoming;
            }
            stmt::Expr::List(existing) => {
                if let stmt::Expr::List(incoming_list) = incoming {
                    existing.items.extend(incoming_list.items);
                } else {
                    existing.items.push(incoming);
                }
            }
            _ => todo!("existing={target:#?}; expr={:#?}", incoming),
        }
    }

    pub(crate) fn merge(&mut self, stmt: Self) {
        self.untyped.merge(stmt.untyped);
    }

    fn expr_mut(&mut self, field: usize) -> &mut stmt::Expr {
        &mut self.current_mut()[field]
    }

    /// Returns the current record being updated
    fn current_mut(&mut self) -> &mut stmt::ExprRecord {
        let values = self.untyped.source.body.expect_values_mut();
        values.rows.last_mut().unwrap().expect_record_mut()
    }

    /// Convert this insert into a list expression.
    ///
    /// The resulting [`Expr<List<M>>`] wraps the insert as a sub-statement,
    /// which can be used as the right-hand side of an association
    /// [`insert`](Association::insert) call or embedded in other expressions.
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
    /// use toasty::stmt::{Insert, Expr, List};
    ///
    /// let insert = Insert::<User>::blank_single();
    /// let _expr: Expr<List<User>> = insert.into_list_expr();
    /// ```
    pub fn into_list_expr(self) -> Expr<List<M>> {
        Expr::from_untyped(stmt::Expr::Stmt(self.untyped.into()))
    }
}

impl<M> Clone for Insert<M> {
    fn clone(&self) -> Self {
        Self {
            untyped: self.untyped.clone(),
            _p: PhantomData,
        }
    }
}

impl<M> From<Insert<M>> for stmt::Expr {
    fn from(value: Insert<M>) -> Self {
        Self::stmt(value.untyped)
    }
}

impl<M> fmt::Debug for Insert<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.untyped.fmt(f)
    }
}
