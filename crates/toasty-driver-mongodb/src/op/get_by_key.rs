use crate::{value, MongoDb};
use toasty_core::{
    driver::{operation::GetByKey, Response},
    schema::db::Schema,
    stmt,
    Result,
};
use std::sync::Arc;
use futures::stream::StreamExt;

pub async fn execute(driver: &MongoDb, schema: &Arc<Schema>, op: GetByKey) -> Result<Response> {
    let table = schema.table(op.table);
    let collection = driver.database.collection::<bson::Document>(&table.name);

    // Build projection for selected columns
    let mut projection = bson::Document::new();
    for column_id in &op.select {
        let column = schema.column(*column_id);
        let field_name = if table.primary_key_columns().any(|pk| pk.id == *column_id) {
            "_id".to_string()
        } else {
            column.name.clone()
        };
        projection.insert(field_name, 1);
    }

    if op.keys.len() == 1 {
        // Single key lookup
        let key = &op.keys[0];
        let filter = build_key_filter(table, key);

        let options = mongodb::options::FindOneOptions::builder()
            .projection(projection)
            .build();

        if let Some(doc) = collection.find_one(filter).with_options(options).await? {
            let row = document_to_record(&doc, &op.select, schema)?;
            Ok(Response::value_stream(stmt::ValueStream::from_value(row)))
        } else {
            Ok(Response::empty_value_stream())
        }
    } else {
        // Multiple keys lookup using $in operator
        let mut key_values = Vec::new();
        for key in &op.keys {
            let bson_key = value::to_bson(key);
            key_values.push(bson_key);
        }

        let mut filter = bson::Document::new();
        filter.insert("_id", bson::doc! { "$in": key_values });

        let options = mongodb::options::FindOptions::builder()
            .projection(projection)
            .build();

        let cursor = collection.find(filter).with_options(options).await?;
        let docs: Vec<_> = cursor.collect().await;

        let schema = schema.clone();
        let select = op.select.clone();

        Ok(Response::value_stream(stmt::ValueStream::from_iter(
            docs.into_iter().map(move |result| {
                result
                    .map_err(|e| anyhow::anyhow!("MongoDB error: {}", e))
                    .and_then(|doc| document_to_record(&doc, &select, &schema))
            }),
        )))
    }
}

fn build_key_filter(table: &toasty_core::schema::db::Table, key: &stmt::Value) -> bson::Document {
    let mut filter = bson::Document::new();

    match key {
        stmt::Value::Record(values) => {
            // Composite key
            for (i, column) in table.primary_key_columns().enumerate() {
                let field_name = if i == 0 { "_id" } else { &column.name };
                filter.insert(field_name, value::to_bson(&values[i]));
            }
        }
        _ => {
            // Single key
            filter.insert("_id", value::to_bson(key));
        }
    }

    filter
}

fn document_to_record(
    doc: &bson::Document,
    select: &[toasty_core::schema::db::ColumnId],
    schema: &Schema,
) -> Result<stmt::Value> {
    let mut values = Vec::new();

    for column_id in select {
        let column = schema.column(*column_id);
        let field_name = if doc.contains_key("_id") && column.name == "_id" {
            "_id"
        } else {
            &column.name
        };

        let bson_value = doc.get(field_name).unwrap_or(&bson::Bson::Null);
        let value = value::from_bson(bson_value, &column.ty);
        values.push(value);
    }

    Ok(stmt::Value::Record(stmt::ValueRecord { fields: values }))
}
