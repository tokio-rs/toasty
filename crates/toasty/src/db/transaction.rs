use std::{ops::Deref, time::Duration};

use toasty_core::driver::operation::Transaction as TransactionOp;
use tokio::time::timeout;

use crate::{db::ConnectionType, engine::Engine, Db};

pub enum Transaction<'a> {
    Root(Db),
    Nested(&'a Db),
}

impl Transaction<'_> {
    pub async fn exec(&self, op: TransactionOp) -> crate::Result<()> {
        match &self.engine.connection {
            ConnectionType::Pool(_) => unreachable!(),
            ConnectionType::Transaction(mutex) => {
                mutex
                    .lock()
                    .await
                    .exec(&self.engine.schema.db, op.into())
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn start(&self) -> crate::Result<()> {
        self.exec(TransactionOp::Start).await
    }

    pub async fn commit(&self) -> crate::Result<()> {
        self.exec(TransactionOp::Commit).await
    }

    pub async fn rollback(&self) -> crate::Result<()> {
        self.exec(TransactionOp::Rollback).await
    }
}

impl Deref for Transaction<'_> {
    type Target = Db;

    fn deref(&self) -> &Self::Target {
        match self {
            Transaction::Root(db) => db,
            Transaction::Nested(db) => db,
        }
    }
}

impl Db {
    pub(crate) async fn begin(&self) -> crate::Result<Transaction<'_>> {
        let tx = match &self.engine.connection {
            ConnectionType::Pool(pool) => {
                let db = Db {
                    driver: self.driver.clone(),
                    engine: Engine::new(
                        self.engine.schema.clone(),
                        ConnectionType::Transaction(pool.get().await?.into()),
                        self.engine.capabilities,
                    ),
                };

                Transaction::Root(db)
            }
            ConnectionType::Transaction(_) => Transaction::Nested(&self),
        };

        tx.start().await?;
        Ok(tx)
    }

    pub async fn transaction<O>(
        &self,
        fut: impl AsyncFnOnce(&Db) -> crate::Result<O>,
    ) -> crate::Result<O> {
        self.transaction_with_timeout(Duration::from_secs(5), fut)
            .await
    }

    pub async fn transaction_with_timeout<O>(
        &self,
        duration: Duration,
        fut: impl AsyncFnOnce(&Db) -> crate::Result<O>,
    ) -> crate::Result<O> {
        let tx = self.begin().await?;

        let Ok(res) = timeout(duration, fut(&tx)).await else {
            tx.rollback().await?;
            return Err(crate::Error::transaction_timed_out(duration));
        };

        match res {
            Ok(res) => {
                tx.commit().await?;
                Ok(res)
            }
            Err(err) => {
                tx.rollback().await?;
                Err(err)
            }
        }
    }
}
