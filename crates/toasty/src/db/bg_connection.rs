use std::sync::Arc;

use toasty_core::{
    driver::{Operation, Response},
    schema::db::Schema,
    stmt::ValueStream,
    Connection, Result,
};
use tokio::sync::{
    mpsc::{self, unbounded_channel},
    oneshot,
};

#[derive(Debug)]
pub(crate) struct BgConnection {
    pub(crate) in_tx: mpsc::UnboundedSender<(
        toasty_core::stmt::Statement,
        oneshot::Sender<Result<ValueStream>>,
    )>,
}

impl BgConnection {
    pub fn new(driver: Box<dyn Connection>) -> Self {
        let (tx, rx) = unbounded_channel();

        tokio::spawn(async move {
            while let Some((stmt, chan)) = rx.recv().await {
                let res = driver.exec(stmt, plan).await;
                chan
            }
        });

        BgConnection { in_tx: tx }
    }

    pub async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response> {
        todo!();
    }
}
