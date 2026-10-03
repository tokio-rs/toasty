//! Runtime dispatch between the embedded `turso` connection and the
//! serverless (`turso_serverless`) HTTP connection.
//!
//! The serverless crate deliberately mirrors the embedded driver's API,
//! so each wrapper here is a two-armed enum whose methods forward
//! verbatim. Values are normalized to [`turso::Value`] at the row
//! boundary and errors are classified into Toasty errors right here, so
//! the rest of the driver is independent of which transport produced a
//! result.

use toasty_core::Result;
use turso::Value as TursoValue;

use crate::error::classify_turso_error;
#[cfg(feature = "serverless")]
use crate::{
    error::classify_serverless_error,
    value::{from_serverless, to_serverless},
};

#[cfg(feature = "serverless")]
fn to_serverless_params(params: Vec<TursoValue>) -> Vec<turso_serverless::Value> {
    params.into_iter().map(to_serverless).collect()
}

pub(crate) enum AnyConn {
    Embedded(turso::Connection),
    #[cfg(feature = "serverless")]
    Serverless(turso_serverless::Connection),
}

impl AnyConn {
    /// Whether this connection talks to a remote serverless database.
    pub(crate) fn is_serverless(&self) -> bool {
        match self {
            Self::Embedded(_) => false,
            #[cfg(feature = "serverless")]
            Self::Serverless(_) => true,
        }
    }

    pub(crate) async fn execute(&self, sql: &str, params: Vec<TursoValue>) -> Result<u64> {
        match self {
            Self::Embedded(conn) => conn
                .execute(sql, params)
                .await
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(conn) => conn
                .execute(sql, to_serverless_params(params))
                .await
                .map_err(classify_serverless_error),
        }
    }

    /// Executes parameterized statements atomically inside a
    /// `BEGIN IMMEDIATE` transaction. On the serverless transport the
    /// whole batch — transaction control included — is one HTTP request.
    /// On failure nothing stays applied, so callers may simply retry.
    pub(crate) async fn transactional_batch(
        &self,
        stmts: &[(String, Vec<TursoValue>)],
    ) -> Result<()> {
        match self {
            Self::Embedded(conn) => {
                conn.execute("BEGIN IMMEDIATE", ())
                    .await
                    .map_err(classify_turso_error)?;
                for (sql, params) in stmts {
                    if let Err(err) = conn.execute(sql, params.clone()).await {
                        let _ = conn.execute("ROLLBACK", ()).await;
                        return Err(classify_turso_error(err));
                    }
                }
                if let Err(err) = conn.execute("COMMIT", ()).await {
                    let _ = conn.execute("ROLLBACK", ()).await;
                    return Err(classify_turso_error(err));
                }
                Ok(())
            }
            #[cfg(feature = "serverless")]
            Self::Serverless(conn) => {
                let stmts = stmts
                    .iter()
                    .map(|(sql, params)| {
                        turso_serverless::BatchStatement::new(
                            sql,
                            to_serverless_params(params.clone()),
                        )
                    })
                    .collect::<turso_serverless::Result<Vec<_>>>()
                    .map_err(classify_serverless_error)?;
                conn.transactional_batch(stmts, turso_serverless::TransactionBehavior::Immediate)
                    .await
                    .map(|_| ())
                    .map_err(classify_serverless_error)
            }
        }
    }

    pub(crate) async fn query(&self, sql: &str, params: Vec<TursoValue>) -> Result<AnyRows> {
        match self {
            Self::Embedded(conn) => conn
                .query(sql, params)
                .await
                .map(AnyRows::Embedded)
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(conn) => conn
                .query(sql, to_serverless_params(params))
                .await
                .map(AnyRows::Serverless)
                .map_err(classify_serverless_error),
        }
    }

    pub(crate) async fn prepare_cached(&self, sql: &str) -> Result<AnyStatement> {
        match self {
            Self::Embedded(conn) => conn
                .prepare_cached(sql)
                .await
                .map(AnyStatement::Embedded)
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(conn) => conn
                .prepare_cached(sql)
                .await
                .map(AnyStatement::Serverless)
                .map_err(classify_serverless_error),
        }
    }

    pub(crate) async fn pragma_update(&self, pragma: &str, value: &str) -> Result<()> {
        match self {
            Self::Embedded(conn) => conn
                .pragma_update(pragma, value)
                .await
                .map(|_| ())
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(conn) => conn
                .pragma_update(pragma, value)
                .await
                .map(|_| ())
                .map_err(classify_serverless_error),
        }
    }
}

pub(crate) enum AnyStatement {
    Embedded(turso::Statement),
    #[cfg(feature = "serverless")]
    Serverless(turso_serverless::Statement),
}

impl AnyStatement {
    pub(crate) async fn execute(&mut self, params: Vec<TursoValue>) -> Result<u64> {
        match self {
            Self::Embedded(stmt) => stmt.execute(params).await.map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(stmt) => stmt
                .execute(to_serverless_params(params))
                .await
                .map_err(classify_serverless_error),
        }
    }

    pub(crate) async fn query(&mut self, params: Vec<TursoValue>) -> Result<AnyRows> {
        match self {
            Self::Embedded(stmt) => stmt
                .query(params)
                .await
                .map(AnyRows::Embedded)
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(stmt) => stmt
                .query(to_serverless_params(params))
                .await
                .map(AnyRows::Serverless)
                .map_err(classify_serverless_error),
        }
    }
}

pub(crate) enum AnyRows {
    Embedded(turso::Rows),
    #[cfg(feature = "serverless")]
    Serverless(turso_serverless::Rows),
}

impl AnyRows {
    pub(crate) async fn next(&mut self) -> Result<Option<AnyRow>> {
        match self {
            Self::Embedded(rows) => rows
                .next()
                .await
                .map(|row| row.map(AnyRow::Embedded))
                .map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(rows) => rows
                .next()
                .await
                .map(|row| row.map(AnyRow::Serverless))
                .map_err(classify_serverless_error),
        }
    }
}

pub(crate) enum AnyRow {
    Embedded(turso::Row),
    #[cfg(feature = "serverless")]
    Serverless(turso_serverless::Row),
}

impl AnyRow {
    pub(crate) fn column_count(&self) -> usize {
        match self {
            Self::Embedded(row) => row.column_count(),
            #[cfg(feature = "serverless")]
            Self::Serverless(row) => row.column_count(),
        }
    }

    pub(crate) fn get_value(&self, index: usize) -> Result<TursoValue> {
        match self {
            Self::Embedded(row) => row.get_value(index).map_err(classify_turso_error),
            #[cfg(feature = "serverless")]
            Self::Serverless(row) => row
                .get_value(index)
                .map(from_serverless)
                .map_err(classify_serverless_error),
        }
    }
}
