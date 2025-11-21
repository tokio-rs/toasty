use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::DeleteByKey, Response},
    schema::db::Schema,
    Result,
};
use std::sync::Arc;

pub async fn execute(
    driver: &MongoDb,
    schema: &Arc<Schema>,
    op: DeleteByKey,
) -> Result<Response> {
    let table = schema.table(op.table);
    let collection = driver.database.collection::<bson::Document>(&table.name);

    if op.keys.len() == 1 {
        // Single key delete
        let key = &op.keys[0];
        let mut filter = bson::Document::new();
        filter.insert("_id", value::to_bson(key));

        let result = collection.delete_one(filter).await?;
        Ok(Response::count(result.deleted_count))
    } else {
        // Multiple keys delete
        let mut key_values = Vec::new();
        for key in &op.keys {
            key_values.push(value::to_bson(key));
        }

        let mut filter = bson::Document::new();
        filter.insert("_id", bson::doc! { "$in": key_values });

        let result = collection.delete_many(filter).await?;
        Ok(Response::count(result.deleted_count))
    }
}
