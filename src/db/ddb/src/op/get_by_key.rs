use super::*;

impl DynamoDB {
    pub(crate) async fn exec_get_by_key<'a>(
        &self,
        schema: &'a schema::Schema,
        op: operation::GetByKey<'a>,
    ) -> Result<stmt::ValueStream<'a>> {
        let table = schema.table(op.table);

        if op.keys.len() == 1 {
            // TODO: set attributes to get
            let res = self
                .client
                .get_item()
                .table_name(self.table_name(table))
                .set_key(Some(ddb_key(&table, &op.keys[0])))
                .send()
                .await?;

            if let Some(item) = res.item() {
                dbg!("DDB: got = {:#?}", item);
                if let Some(filter) = op.post_filter {
                    // TODO: improve filtering logic
                    let row = item_to_record(item, table.columns.iter())?;
                    if filter.eval_bool(&row)? {
                        let row = stmt::Record::from_vec(
                            op.select.iter().map(|id| row[id.index].clone()).collect(),
                        );
                        Ok(stmt::ValueStream::from_value(row))
                    } else {
                        Ok(stmt::ValueStream::new())
                    }
                } else {
                    let row = item_to_record(item, op.select.iter().map(|id| schema.column(id)))?;
                    Ok(stmt::ValueStream::from_value(row))
                }
            } else {
                Ok(stmt::ValueStream::new())
            }
        } else {
            if op.keys.len() > 100 {
                todo!("fetching over 100 keys not yet supported");
            }

            let mut keys = vec![];

            for key in &op.keys {
                keys.push(ddb_key(&table, key));
            }

            let res = self
                .client
                .batch_get_item()
                .set_request_items(Some({
                    let mut items = HashMap::new();
                    items.insert(
                        self.table_name(table),
                        KeysAndAttributes::builder()
                            .set_keys(Some(keys))
                            .build()
                            .unwrap(),
                    );
                    items
                }))
                .send()
                .await?;

            let Some(mut responses) = res.responses else {
                return Ok(stmt::ValueStream::new());
            };
            let Some(items) = responses.remove(&self.table_name(table)) else {
                return Ok(stmt::ValueStream::new());
            };

            Ok(stmt::ValueStream::from_iter(items.into_iter().filter_map(
                move |item| {
                    let row = match item_to_record(
                        &item,
                        op.select.iter().map(|column_id| schema.column(column_id)),
                    ) {
                        Ok(row) => row,
                        Err(e) => return Some(Err(e)),
                    };

                    if let Some(filter) = &op.post_filter {
                        match filter.eval_bool(&row) {
                            Ok(true) => {}
                            Ok(false) => return None,
                            Err(e) => return Some(Err(e)),
                        }
                    }

                    Some(Ok(row))
                },
            )))
        }
    }
}
