use crate::{
    Error, Executor, Result,
    schema::Model,
    stmt::{Association, Expr, List, Query},
};

/// Deferred insert into a has-many relation. Returned by the generated
/// `insert()` method on a relation-scoped `Query<List<M>>`. On execution
/// this will add an item to the relation this query was scoped from.
///
/// # Execution
///
/// Call [`exec`](RelationInsert::exec) to run the insert.
pub struct RelationInsert<M: Model> {
    pub(crate) query: Query<List<M>>,
    pub(crate) item: Expr<M>,
}

impl<M: Model> RelationInsert<M> {
    /// Execute this insert statement of a relation against the given executor.
    ///
    /// Returns `Ok(())` on success. The given item will be inserted as a relation of the parent
    /// object into the database.
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
    ///     .exec(&mut db)
    ///     .await.unwrap();
    /// let todo = Todo::create().title("dummy").exec(&mut db).await.unwrap();
    /// user.todos().insert(&todo).exec(&mut db).await.unwrap();
    /// # });
    /// ```
    pub async fn exec(mut self, executor: &mut dyn Executor) -> Result<()> {
        match self.query.take_via_assoc() {
            Some(untyped) if untyped.path.projection.as_slice().len() == 1 => {
                let assoc = Association::<List<M>>::from_untyped(untyped);
                executor.exec(assoc.insert(self.item)).await
            }
            Some(_) => Err(Error::unsupported_feature(
                "insert is not supported on multi-step relation traversals",
            )),
            None => Err(Error::unsupported_feature(
                "insert requires a relation-scoped query",
            )),
        }
    }
}
