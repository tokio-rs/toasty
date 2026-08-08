use crate::{
    Error, Executor, Result,
    schema::Model,
    stmt::{Association, Expr, List, Query},
};

/// Deferred remove of a has-many relation. Returned by the generated
/// `insert()` method on a relation-scoped `Query<List<M>>`. On execution
/// this will remove an item from the relation this query was scoped from.
///
/// # Execution
///
/// Call [`exec`](RelationRemove::exec) to run the removal.
pub struct RelationRemove<M: Model> {
    pub(crate) query: Query<List<M>>,
    pub(crate) item: Expr<M>,
}

impl<M: Model> RelationRemove<M> {
    /// Execute this remove statement of a relation against the given executor.
    ///
    /// Returns `Ok(())` on success. The given item will be removed as a relation fromm the parent
    /// object in the database.
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
    /// #     name: String,
    /// #     #[has_many]
    /// #     todos: toasty::Deferred<Vec<Todo>>,
    /// # }
    /// # #[derive(Debug, toasty::Model)]
    /// # struct Todo {
    /// #     #[key]
    /// #     #[auto]
    /// #     id: i64,
    /// #     title: String,
    /// #     #[index]
    /// #     user_id: Option<i64>,
    /// #     #[belongs_to(key = user_id, references = id)]
    /// #     user: toasty::Deferred<Option<User>>,
    /// # }
    /// # let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// # let mut db = toasty::Db::builder().models(toasty::models!(User)).build(driver).await.unwrap();
    /// # db.push_schema().await.unwrap();
    /// let user = User::create()
    ///     .name("John")
    ///     .todos([Todo::create().title("dummy one")])
    ///     .todos([Todo::create().title("dummy two")])
    ///     .exec(&mut db)
    ///     .await
    ///     .unwrap();
    /// let todos: Vec<_> = user.todos().exec(&mut db).await.unwrap();
    /// user.todos().remove(&todos[0]).exec(&mut db).await.unwrap();
    /// # });
    /// ```
    pub async fn exec(mut self, executor: &mut dyn Executor) -> Result<()> {
        match self.query.take_via_assoc() {
            Some(untyped) if untyped.path.projection.as_slice().len() == 1 => {
                let assoc = Association::<List<M>>::from_untyped(untyped);
                executor.exec(assoc.remove(self.item)).await
            }
            Some(_) => Err(Error::unsupported_feature(
                "remove is not supported on multi-step relation traversals",
            )),
            None => Err(Error::unsupported_feature(
                "remove requires a relation-scoped query",
            )),
        }
    }
}
