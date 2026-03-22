use crate::{
    schema::Model,
    stmt::{self, IntoExpr, IntoInsert, List},
    Executor, Result,
};
use toasty_core::stmt as core_stmt;

/// A builder for inserting multiple records of the same model in a single
/// statement.
///
/// Records are accumulated with [`item`](CreateMany::item) or
/// [`with_item`](CreateMany::with_item), then executed with
/// [`exec`](CreateMany::exec). Alternatively, call
/// [`into_expr`](CreateMany::into_expr) to embed the batch insert as an
/// expression inside another statement (e.g., a has-many association insert).
///
/// # Examples
///
/// ```no_run
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     #[auto]
/// #     id: i64,
/// #     name: String,
/// # }
/// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
/// # let mut db = toasty::Db::builder().register::<User>().build(driver).await.unwrap();
/// # db.push_schema().await.unwrap();
/// use toasty::CreateMany;
///
/// let users = CreateMany::<User>::new()
///     .with_item(|u| u.name("Alice"))
///     .with_item(|u| u.name("Bob"))
///     .exec(&mut db)
///     .await
///     .unwrap();
/// assert_eq!(users.len(), 2);
/// # });
/// ```
pub struct CreateMany<M: Model> {
    stmts: Vec<stmt::Insert<M>>,
}

impl<M: Model> CreateMany<M> {
    /// Create an empty `CreateMany` builder with no records queued.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record from any value that implements [`IntoInsert`](stmt::IntoInsert)
    /// for model `M`.
    ///
    /// Returns `self` for method chaining.
    pub fn item(mut self, item: impl stmt::IntoInsert<Model = M>) -> Self {
        let stmt = item.into_insert();
        assert!(
            stmt.untyped.source.single,
            "BUG: insert statement should have `single` flag set"
        );
        self.stmts.push(stmt);
        self
    }

    /// Append a record using a closure that configures the model's generated
    /// create builder.
    ///
    /// `f` receives a default create builder and must return it after setting
    /// the desired fields.
    ///
    /// Returns `self` for method chaining.
    pub fn with_item(mut self, f: impl FnOnce(M::Create) -> M::Create) -> Self {
        let create = f(M::Create::default());
        let stmt = create.into_insert();
        assert!(
            stmt.untyped.source.single,
            "BUG: insert statement should have `single` flag set"
        );
        self.stmts.push(stmt);
        self
    }

    /// Convert the collected inserts into a list expression suitable for
    /// embedding in a parent insert statement (e.g., as a nested HasMany value).
    ///
    /// Unlike `exec`, this does not run any database query.
    pub fn into_expr(self) -> stmt::Expr<List<M>> {
        if self.stmts.is_empty() {
            return stmt::Expr::from_untyped(core_stmt::Expr::list(std::iter::empty::<
                core_stmt::Expr,
            >()));
        }
        let mut stmts = self.stmts.into_iter();
        let mut merged = stmts.next().unwrap();
        for stmt in stmts {
            merged.merge(stmt);
        }
        // Clear the single flag so the engine handles multi-row inserts correctly.
        merged.untyped.source.single = false;
        merged.into_list_expr()
    }

    /// Execute the batch insert and return the created records.
    ///
    /// Returns an empty `Vec` if no records were queued.
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<Vec<M>> {
        if self.stmts.is_empty() {
            return Ok(vec![]);
        }

        let mut stmts = self.stmts.into_iter();
        let mut merged = stmts.next().unwrap();

        for stmt in stmts {
            merged.merge(stmt);
        }

        merged.untyped.source.single = false;

        let stmt: crate::Statement<List<M>> =
            crate::Statement::from_untyped_stmt(merged.untyped.into());
        executor.exec(stmt).await
    }
}

impl<M: Model> IntoExpr<List<M>> for CreateMany<M> {
    fn into_expr(self) -> stmt::Expr<List<M>> {
        self.into_expr()
    }

    fn by_ref(&self) -> stmt::Expr<List<M>> {
        todo!()
    }
}

impl<M: Model> Default for CreateMany<M> {
    fn default() -> Self {
        Self { stmts: vec![] }
    }
}
