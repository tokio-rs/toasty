use super::{Expr, IntoExpr, IntoStatement, Statement};
use crate::schema::Load;
use crate::{Executor, Result};

use toasty_core::stmt;

/// A batch of queries composed into a single statement.
///
/// Created by [`batch()`]. The composed statement flows through
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
pub fn batch<Q: IntoStatement>(queries: Q) -> Batch<Q::Returning>
where
    Q::Returning: Load,
{
    Batch {
        stmt: queries.into_statement(),
    }
}

impl<T: Load> Batch<T> {
    /// Execute the batched queries and return the deserialized results.
    pub async fn exec(self, executor: &mut dyn Executor) -> Result<T::Output> {
        executor.exec(self.stmt).await
    }
}

impl<T> From<Statement<T>> for Batch<T> {
    fn from(stmt: Statement<T>) -> Self {
        Batch { stmt }
    }
}

impl<T> IntoExpr<T> for Batch<T> {
    fn into_expr(self) -> Expr<T> {
        Expr::from_untyped(stmt::Expr::stmt(self.stmt.untyped))
    }

    fn by_ref(&self) -> Expr<T> {
        Expr::from_untyped(stmt::Expr::stmt(self.stmt.untyped.clone()))
    }
}
