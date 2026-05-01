use crate::sort_key_columns;

use super::{
    Connection, KeysAndAttributes, Result, Schema, ddb_key, item_to_record, operation, stmt,
};
use std::{collections::HashMap, sync::Arc};
use toasty_core::driver::ExecResponse;

impl Connection {
    pub(crate) async fn exec_get_by_key(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::GetByKey,
    ) -> Result<ExecResponse> {
        let table = schema.db.table(op.table);
        let sk_cols = sort_key_columns(table);

        if op.keys.len() == 1 {
            // TODO: set attributes to get
            tracing::trace!(key = ?op.keys[0], table_name = %table.name, "getting single item");
            let res = self
                .client
                .get_item()
                .table_name(&table.name)
                .set_key(Some(ddb_key(table, &op.keys[0])))
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;

            if let Some(item) = res.item() {
                let row = item_to_record(
                    item,
                    op.select.iter().map(|id| schema.db.column(*id)),
                    &sk_cols,
                )?;
                Ok(ExecResponse::value_stream(stmt::ValueStream::from_value(
                    row,
                )))
            } else {
                Ok(ExecResponse::empty_value_stream())
            }
        } else {
            if op.keys.len() > 100 {
                todo!("fetching over 100 keys not yet supported");
            }

            let mut keys = vec![];

            for key in &op.keys {
                keys.push(ddb_key(table, key));
            }
            tracing::trace!(key_count = op.keys.len(), table_name = %table.name, "batch getting items");

            let res = self
                .client
                .batch_get_item()
                .set_request_items(Some({
                    let mut items = HashMap::new();
                    items.insert(
                        table.name.clone(),
                        KeysAndAttributes::builder()
                            .set_keys(Some(keys))
                            .build()
                            .unwrap(),
                    );
                    items
                }))
                .send()
                .await
                .map_err(toasty_core::Error::driver_operation_failed)?;

            let Some(mut responses) = res.responses else {
                return Ok(ExecResponse::empty_value_stream());
            };
            let Some(items) = responses.remove(&table.name) else {
                return Ok(ExecResponse::empty_value_stream());
            };

            let schema = schema.clone();

            Ok(ExecResponse::value_stream(stmt::ValueStream::from_iter(
                items.into_iter().map(move |item| {
                    item_to_record(
                        &item,
                        op.select
                            .iter()
                            .map(|column_id| schema.db.column(*column_id)),
                        &sk_cols,
                    )
                }),
            )))
        }
    }
}
