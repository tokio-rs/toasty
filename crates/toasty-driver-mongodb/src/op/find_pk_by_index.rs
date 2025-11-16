use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::FindPkByIndex, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;
use futures::stream::StreamExt;

pub async fn execute(
    driver: &MongoDb,
    schema: &Arc<Schema>,
    op: FindPkByIndex,
) -> Result<Response> {
    let table = schema.table(op.table);
    let collection = driver.database.collection::<bson::Document>(&table.name);

    // Build filter from the index query
    let mut filter = bson::Document::new();

    // For now, simple implementation - will need full expression conversion
    // TODO: Convert op.filter to MongoDB query
    match &op.filter {
        stmt::Expr::Value(val) => {
            // Simple case: querying by a single value on the index
            let index = &table.indices[op.index.index];
            if let Some(column_ref) = index.columns.first() {
                let column = table.column(column_ref.column);
                filter.insert(&column.name, value::to_bson(val));
            }
        }
        _ => todo!("Complex index filter expressions: {:?}", op.filter),
    }

    // Only select primary key columns
    let mut projection = bson::Document::new();
    projection.insert("_id", 1);

    let options = mongodb::options::FindOptions::builder()
        .projection(projection)
        .build();

    let cursor = collection.find(filter).with_options(options).await?;
    let docs: Vec<_> = cursor.collect().await;

    // Get primary key type before moving into closure
    let pk_column = table.primary_key_columns().next().unwrap();
    let pk_type = pk_column.ty.clone();

    // Return only primary keys
    Ok(Response::value_stream(stmt::ValueStream::from_iter(
        docs.into_iter().map(move |result| {
            result
                .map_err(|e| anyhow::anyhow!("MongoDB error: {}", e))
                .and_then(|doc| {
                    // Extract primary key value
                    let bson_value = doc.get("_id").unwrap_or(&bson::Bson::Null);
                    Ok(value::from_bson(bson_value, &pk_type))
                })
        }),
    )))
}
