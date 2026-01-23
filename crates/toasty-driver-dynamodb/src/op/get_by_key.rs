use super::{
    ddb_key, item_to_record, operation, stmt, Connection, KeysAndAttributes, Result, Schema,
};
use std::{collections::HashMap, sync::Arc};
use toasty_core::driver::Response;

impl Connection {
    pub(crate) async fn exec_get_by_key(
        &mut self,
        schema: &Arc<Schema>,
        op: operation::GetByKey,
    ) -> Result<Response> {
        let table = schema.table(op.table);

        if op.keys.len() == 1 {
            // TODO: set attributes to get
            let res = self
                .client
                .get_item()
                .table_name(&table.name)
                .set_key(Some(ddb_key(table, &op.keys[0])))
                .send()
                .await
                .map_err(toasty_core::Error::driver)?;

            if let Some(item) = res.item() {
                let row = item_to_record(item, op.select.iter().map(|id| schema.column(*id)))?;
                Ok(Response::value_stream(stmt::ValueStream::from_value(row)))
            } else {
                Ok(Response::empty_value_stream())
            }
        } else {
            if op.keys.len() > 100 {
                todo!("fetching over 100 keys not yet supported");
            }

            let mut keys = vec![];

            for key in &op.keys {
                keys.push(ddb_key(table, key));
            }

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
                .map_err(toasty_core::Error::driver)?;

            let Some(mut responses) = res.responses else {
                return Ok(Response::empty_value_stream());
            };
            let Some(items) = responses.remove(&table.name) else {
                return Ok(Response::empty_value_stream());
            };

            let schema = schema.clone();

            Ok(Response::value_stream(stmt::ValueStream::from_iter(
                items.into_iter().map(move |item| {
                    item_to_record(
                        &item,
                        op.select.iter().map(|column_id| schema.column(*column_id)),
                    )
                }),
            )))
        }
    }
}
