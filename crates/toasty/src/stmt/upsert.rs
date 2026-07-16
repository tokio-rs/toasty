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
/// An ordinary field setter initializes the field when the row is absent and
/// applies the same assignment when the selected constraint matches. The
/// branch-specific closures override that behavior for fields that need
/// different create and update values. A regular execution returns the record
/// stored by the database; `or_ignore` returns `Some(M)` after an insert and
/// `None` after a conflict.
///
/// The database executes the conflict check and mutation atomically. Backend
/// support differs: PostgreSQL, SQLite, and Turso accept primary-key and unique
/// targets plus branch-specific assignments; DynamoDB accepts primary-key
/// targets without branch-specific assignments; MySQL does not support this
/// targeted upsert API.
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
            assignments: stmt::Assignments::new(),
            on_create: None,
            on_update: None,
            create_defaults: stmt::Assignments::new(),
            action: stmt::UpsertAction::Update,
            invalid_shared_assignments: Vec::new(),
        }));
        Self {
            untyped: insert,
            _p: PhantomData,
        }
    }

    /// Sets a value on both the create and update branches.
    #[doc(hidden)]
    pub fn set_shared(&mut self, field: usize, expr: stmt::Expr) {
        self.assign_shared(field, |assignments, projection| {
            assignments.set(projection, expr);
        });
    }

    /// Applies an assignment to every branch without an explicit override.
    #[doc(hidden)]
    pub fn assign_shared(
        &mut self,
        field: usize,
        assign: impl FnOnce(&mut stmt::Assignments, stmt::Projection),
    ) {
        let create_overridden = self
            .untyped
            .upsert
            .as_ref()
            .unwrap()
            .on_create
            .as_ref()
            .is_some_and(|clause| clause.assignments.contains(&[field]));

        assign(
            self.update_assignments_mut(),
            stmt::Projection::from_index(field),
        );
        if !create_overridden {
            self.sync_create_from_update(field);
        }
    }

    /// Derives the create value from an assignment already set on the update branch.
    #[doc(hidden)]
    pub fn sync_create_from_update(&mut self, field: usize) {
        let projection = stmt::Projection::from_index(field);
        let assignment = self
            .untyped
            .upsert
            .as_ref()
            .unwrap()
            .assignments
            .get(&projection)
            .cloned();

        let create = assignment.as_ref().and_then(create_expr_for_assignment);

        let invalid = &mut self
            .untyped
            .upsert
            .as_mut()
            .unwrap()
            .invalid_shared_assignments;
        invalid.retain(|candidate| candidate != &projection);

        if let Some(create) = create {
            self.set_create(field, create);
        } else {
            invalid.push(projection);
        }
    }

    /// Starts an explicit create branch.
    #[doc(hidden)]
    pub fn begin_on_create(&mut self) {
        self.untyped
            .upsert
            .as_mut()
            .unwrap()
            .on_create
            .get_or_insert_default();
    }

    /// Starts an explicit update branch.
    #[doc(hidden)]
    pub fn begin_on_update(&mut self) {
        self.untyped
            .upsert
            .as_mut()
            .unwrap()
            .on_update
            .get_or_insert_default();
    }

    /// Sets a value only on the create branch.
    #[doc(hidden)]
    pub fn set_create(&mut self, field: usize, expr: stmt::Expr) {
        let values = self.untyped.source.body.as_values_mut_unwrap();
        let row = values.rows.last_mut().unwrap().as_record_mut_unwrap();
        row.fields[field] = expr;
        let upsert = self.untyped.upsert.as_mut().unwrap();
        upsert.create_defaults.unset(&[field]);
        upsert
            .invalid_shared_assignments
            .retain(|projection| projection.as_slice() != [field]);
    }

    /// Overrides a field only on the create branch.
    #[doc(hidden)]
    pub fn set_create_override(&mut self, field: usize, expr: stmt::Expr) {
        let upsert = self.untyped.upsert.as_mut().unwrap();
        upsert
            .on_create
            .get_or_insert_default()
            .assignments
            .set(stmt::Projection::from_index(field), expr);
        upsert.create_defaults.unset(&[field]);
        upsert
            .invalid_shared_assignments
            .retain(|projection| projection.as_slice() != [field]);
    }

    /// Records a create default for DynamoDB's `if_not_exists` lowering.
    #[doc(hidden)]
    pub fn set_create_default(&mut self, field: usize, expr: stmt::Expr) {
        self.set_create(field, expr.clone());
        self.untyped
            .upsert
            .as_mut()
            .unwrap()
            .create_defaults
            .set(stmt::Projection::from_index(field), expr);
    }

    /// Returns the update-branch assignment map.
    #[doc(hidden)]
    pub fn update_assignments_mut(&mut self) -> &mut stmt::Assignments {
        &mut self.untyped.upsert.as_mut().unwrap().assignments
    }

    /// Overrides a field only on the update branch.
    #[doc(hidden)]
    pub fn assign_update_override(
        &mut self,
        field: usize,
        assign: impl FnOnce(&mut stmt::Assignments, stmt::Projection),
    ) {
        let assignments = &mut self
            .untyped
            .upsert
            .as_mut()
            .unwrap()
            .on_update
            .get_or_insert_default()
            .assignments;
        assign(assignments, stmt::Projection::from_index(field));
    }

    /// Changes conflict handling to `DO NOTHING`.
    #[doc(hidden)]
    pub fn set_ignore(&mut self) {
        let upsert = self.untyped.upsert.as_mut().unwrap();
        upsert.action = stmt::UpsertAction::Ignore;
        upsert.assignments = stmt::Assignments::new();
        upsert.on_update = None;
    }

    /// Consumes the wrapper and returns its untyped statement.
    #[doc(hidden)]
    pub fn into_untyped(self) -> stmt::Statement {
        self.untyped.into()
    }
}

fn create_expr_for_assignment(assignment: &stmt::Assignment) -> Option<stmt::Expr> {
    match assignment {
        stmt::Assignment::Set(expr)
        | stmt::Assignment::Append(expr)
        | stmt::Assignment::Add(expr) => Some(expr.clone()),
        stmt::Assignment::Subtract(stmt::Expr::Value(value)) => {
            negate_numeric(value).map(stmt::Expr::Value)
        }
        _ => None,
    }
}

fn negate_numeric(value: &stmt::Value) -> Option<stmt::Value> {
    match value {
        stmt::Value::I8(value) => value.checked_neg().map(stmt::Value::I8),
        stmt::Value::I16(value) => value.checked_neg().map(stmt::Value::I16),
        stmt::Value::I32(value) => value.checked_neg().map(stmt::Value::I32),
        stmt::Value::I64(value) => value.checked_neg().map(stmt::Value::I64),
        stmt::Value::F32(value) => Some(stmt::Value::F32(-value)),
        stmt::Value::F64(value) => Some(stmt::Value::F64(-value)),
        _ => None,
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
