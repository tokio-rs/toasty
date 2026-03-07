use crate::stmt::IntoStatement;
use crate::{Executor, ExecutorExt, Load, Result, Statement};

/// A batch of queries composed into a single statement.
///
/// Created by [`batch()`](crate::batch). The composed statement flows through
/// the standard engine pipeline and the result is deserialized via [`Load`].
pub struct Batch<T> {
    stmt: Statement<T>,
}

/// Compose multiple independent queries into a single batched statement.
///
/// The queries are sent to the database in a single round-trip and the results
/// come back as a typed tuple matching the input queries.
///
/// # Examples
///
/// ```ignore
/// let (active_users, recent_posts) = toasty::batch((
///     User::find_by_active(true),
///     Post::find_recent(100),
/// )).exec(&mut db).await?;
/// ```
pub fn batch<T, Q: IntoStatement<T>>(queries: Q) -> Batch<T>
where
    T: Load,
{
    Batch {
        stmt: queries.into_statement(),
    }
}

impl<T: Load> Batch<T> {
    /// Execute the batched queries and return the deserialized results.
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T> {
        let value = executor.exec_one(self.stmt).await?;
        T::load(value)
    }
}
